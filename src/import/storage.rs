//! A configuration edit owns the same sidecar as runners, then its edit sidecar.
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use crate::run_lock::RunLock;

const LIMIT: usize = 1024 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    device: u64,
    inode: u64,
    mode: u32,
    links: u64,
    modified: (i64, i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Snapshot {
    pub text: String,
    stamp: Stamp,
}

pub(super) fn read(path: &Path) -> Result<Option<Snapshot>, String> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Cannot open import destination: {error}")),
    };
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() > LIMIT as u64 {
        return Err(
            "Import destination must be a regular configuration file of at most 1 MiB.".into(),
        );
    }
    let mut text = String::new();
    file.take(LIMIT as u64 + 1)
        .read_to_string(&mut text)
        .map_err(|_| "Cannot read the import destination as UTF-8.".to_owned())?;
    if text.len() > LIMIT {
        return Err("Import destination exceeds 1 MiB.".into());
    }
    Ok(Some(Snapshot {
        text,
        stamp: Stamp {
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
            links: metadata.nlink(),
            modified: (metadata.mtime(), metadata.mtime_nsec()),
        },
    }))
}

/// Resolve existing symlink parents before normalizing missing path components.
/// The original spelling is retained separately and checked again at publication.
pub(super) fn identity(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| error.to_string())?
            .join(path)
    };
    let mut result = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::RootDir => result.push("/"),
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            Component::Normal(name) => {
                result.push(name);
                match fs::symlink_metadata(&result) {
                    Ok(_) => {
                        result = result.canonicalize().map_err(|error| {
                            format!("Cannot resolve import destination: {error}")
                        })?
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(format!("Cannot inspect import destination: {error}"));
                    }
                }
            }
            Component::Prefix(_) => return Err("Unsupported import destination prefix.".into()),
        }
    }
    if result.file_name().is_none() {
        return Err("Select a configuration filename.".into());
    }
    Ok(result)
}

pub(super) struct Transaction {
    requested: PathBuf,
    target: PathBuf,
    original: Option<Snapshot>,
    parent: File,
    _run: Arc<RunLock>,
    _edit: File,
}

impl Transaction {
    pub fn begin(
        requested: &Path,
        target: &Path,
        original: &Option<Snapshot>,
    ) -> Result<Self, String> {
        Self::begin_with_owner(requested, target, original, None)
    }

    pub fn begin_owned(
        requested: &Path,
        target: &Path,
        original: &Option<Snapshot>,
        run: Arc<RunLock>,
    ) -> Result<Self, String> {
        if !run.owns(target) {
            return Err("Configuration owner does not match destination.".into());
        }
        Self::begin_with_owner(requested, target, original, Some(run))
    }

    fn begin_with_owner(
        requested: &Path,
        target: &Path,
        original: &Option<Snapshot>,
        owner: Option<Arc<RunLock>>,
    ) -> Result<Self, String> {
        if original
            .as_ref()
            .is_some_and(|snapshot| snapshot.stamp.links != 1)
        {
            return Err("Import refuses a hard-linked configuration; its identity would split on replacement.".into());
        }
        if identity(requested)? != target {
            return Err("Import destination changed after review; preview again.".into());
        }
        let parent = target.parent().ok_or("Import destination has no parent.")?;
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)
            .map_err(|error| format!("Cannot create import destination directory: {error}"))?;
        if identity(requested)? != target {
            return Err("Import destination changed while creating its parent.".into());
        }
        let parent_file = File::open(parent)
            .map_err(|error| format!("Cannot retain import parent identity: {error}"))?;
        let run = match owner {
            Some(owner) => owner,
            None => Arc::new(RunLock::try_acquire_target(target)?.ok_or(
                "This workspace is running or another import is applying. Preview is available. Shut down this workspace, apply the import, then reopen it.")?),
        };
        let mut sidecar = target.as_os_str().to_os_string();
        sidecar.push(".devctl.edit.lock");
        let edit = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
            .open(PathBuf::from(sidecar))
            .map_err(|error| format!("Cannot open import edit lock: {error}"))?;
        if !edit
            .metadata()
            .map_err(|error| error.to_string())?
            .is_file()
        {
            return Err("Import edit lock must be a regular file.".into());
        }
        edit.try_lock()
            .map_err(|_| "Another import owns this destination; preview again.".to_owned())?;
        let transaction = Self {
            requested: requested.into(),
            target: target.into(),
            original: original.clone(),
            parent: parent_file,
            _run: run,
            _edit: edit,
        };
        transaction.recheck()?;
        Ok(transaction)
    }

    fn recheck(&self) -> Result<(), String> {
        let parent = self.target.parent().expect("validated parent");
        let expected = self.parent.metadata().map_err(|error| error.to_string())?;
        let observed = fs::metadata(parent).map_err(|error| error.to_string())?;
        if expected.dev() != observed.dev() || expected.ino() != observed.ino() {
            return Err(
                "Import destination parent changed after review; no configuration was written."
                    .into(),
            );
        }
        if identity(&self.requested)? != self.target || read(&self.target)? != self.original {
            return Err("Import destination changed after review; no configuration was written. Preview again.".into());
        }
        Ok(())
    }

    pub fn publish(&self, text: &str) -> Result<(), String> {
        if text.len() > LIMIT {
            return Err("Proposed configuration exceeds 1 MiB.".into());
        }
        self.recheck()?;
        let parent = self.target.parent().expect("validated parent");
        let path = parent.join(format!(
            ".devctl-import-{}-{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .map_err(|error| format!("Cannot stage import: {error}"))?;
        struct Temporary(PathBuf);
        impl Drop for Temporary {
            fn drop(&mut self) {
                let _ = fs::remove_file(&self.0);
            }
        }
        let _temporary = Temporary(path.clone());
        file.write_all(text.as_bytes())
            .map_err(|error| format!("Cannot write import staging: {error}"))?;
        let mode = self
            .original
            .as_ref()
            .map_or(0o600, |original| original.stamp.mode);
        file.set_permissions(fs::Permissions::from_mode(mode))
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("Cannot sync import staging: {error}"))?;
        self.recheck()?;
        if self.original.is_some() {
            fs::rename(&path, &self.target)
                .map_err(|error| format!("Cannot publish import: {error}"))?;
        } else {
            // Same-directory hard-link publication is atomic and cannot overwrite
            // a non-cooperating creator. The staging name is removed by the guard.
            fs::hard_link(&path, &self.target).map_err(|error| {
                format!("Cannot publish new import without replacement: {error}")
            })?;
            fs::remove_file(&path).map_err(|error| {
                format!("Configuration was saved, but staging cleanup failed: {error}")
            })?;
        }
        File::open(parent)
            .and_then(|file| file.sync_all())
            .map_err(|error| format!("Configuration was saved, but directory sync failed: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = PathBuf::from("/tmp").join(format!(
                "devctl-edit-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
        fn path(&self) -> PathBuf {
            self.0.join("config.toml")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn transaction_holds_runner_and_editor_ownership_through_atomic_replacement() {
        let fixture = Fixture::new();
        let path = fixture.path();
        fs::write(&path, "old bytes").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        let original = read(&path).unwrap();
        let target = identity(&path).unwrap();
        let transaction = Transaction::begin(&path, &target, &original).unwrap();
        assert!(RunLock::try_acquire(&path).unwrap().is_none());
        assert!(Transaction::begin(&path, &target, &original).is_err());
        transaction.publish("complete replacement\n").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "complete replacement\n");
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o640);
        assert!(RunLock::try_acquire(&path).unwrap().is_none());
        drop(transaction);
        assert!(RunLock::try_acquire(&path).unwrap().is_some());
    }

    #[test]
    fn concurrent_external_edits_and_creators_are_never_overwritten() {
        for existing in [false, true] {
            let fixture = Fixture::new();
            let path = fixture.path();
            if existing {
                fs::write(&path, "reviewed").unwrap();
            }
            let original = read(&path).unwrap();
            let target = identity(&path).unwrap();
            let transaction = Transaction::begin(&path, &target, &original).unwrap();
            fs::write(&path, "external content").unwrap();
            assert!(
                transaction
                    .publish("proposed")
                    .unwrap_err()
                    .contains("changed after review")
            );
            assert_eq!(fs::read_to_string(&path).unwrap(), "external content");
            assert!(!fs::read_dir(&fixture.0).unwrap().any(|entry| {
                entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".devctl-import-")
            }));
        }
        let fixture = Fixture::new();
        let parent = fixture.0.join("parent");
        fs::create_dir(&parent).unwrap();
        let path = parent.join("config.toml");
        let target = identity(&path).unwrap();
        let transaction = Transaction::begin(&path, &target, &None).unwrap();
        fs::rename(&parent, fixture.0.join("moved-parent")).unwrap();
        fs::create_dir(&parent).unwrap();
        assert!(
            transaction
                .publish("proposed")
                .unwrap_err()
                .contains("parent changed")
        );
        assert!(!path.exists());
    }

    #[test]
    fn alias_retargeting_and_hardlinks_preserve_both_original_files() {
        let fixture = Fixture::new();
        let path = fixture.path();
        let other = fixture.0.join("other.toml");
        let alias = fixture.0.join("alias.toml");
        fs::write(&path, "first").unwrap();
        fs::write(&other, "second").unwrap();
        symlink(&path, &alias).unwrap();
        let original = read(&path).unwrap();
        let target = identity(&alias).unwrap();
        let transaction = Transaction::begin(&alias, &target, &original).unwrap();
        fs::remove_file(&alias).unwrap();
        symlink(&other, &alias).unwrap();
        assert!(transaction.publish("proposed").is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "first");
        assert_eq!(fs::read_to_string(&other).unwrap(), "second");
        drop(transaction);
        let hardlink = fixture.0.join("hardlink.toml");
        fs::hard_link(&path, &hardlink).unwrap();
        let linked = read(&path).unwrap();
        assert!(
            Transaction::begin(&path, &target, &linked)
                .err()
                .unwrap()
                .contains("hard-linked")
        );
        assert_eq!(fs::read_to_string(hardlink).unwrap(), "first");
    }

    #[test]
    fn destination_reader_rejects_special_oversized_and_non_utf8_files_without_blocking() {
        let fixture = Fixture::new();
        let path = fixture.path();
        nix::unistd::mkfifo(
            &path,
            nix::sys::stat::Mode::S_IRUSR | nix::sys::stat::Mode::S_IWUSR,
        )
        .unwrap();
        assert!(read(&path).unwrap_err().contains("regular configuration"));
        fs::remove_file(&path).unwrap();
        fs::write(&path, vec![b' '; LIMIT]).unwrap();
        assert!(read(&path).is_ok());
        fs::write(&path, vec![b' '; LIMIT + 1]).unwrap();
        assert!(read(&path).is_err());
        fs::write(&path, [0xff]).unwrap();
        assert!(read(&path).unwrap_err().contains("UTF-8"));
    }
}
