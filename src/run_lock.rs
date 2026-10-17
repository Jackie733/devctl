use std::{
    fs::{File, OpenOptions, TryLockError},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

pub struct RunLock {
    _file: File,
    target: PathBuf,
}

impl RunLock {
    pub(crate) fn owns(&self, target: &Path) -> bool {
        self.target == target
    }

    pub fn acquire(path: &Path) -> Result<Self, String> {
        let config_path = path.canonicalize().map_err(|error| {
            format!("Failed to resolve config path {}: {error}", path.display())
        })?;
        Self::try_acquire(&config_path)?
            .ok_or_else(|| format!("Config '{}' is already managed.", config_path.display()))
    }

    pub fn try_acquire(path: &Path) -> Result<Option<Self>, String> {
        let config_path = path.canonicalize().map_err(|error| {
            format!("Failed to resolve config path {}: {error}", path.display())
        })?;
        Self::try_acquire_target(&config_path)
    }

    /// Caller supplies a canonical parent/name, possibly before first publication.
    pub fn try_acquire_target(config_path: &Path) -> Result<Option<Self>, String> {
        let mut name = config_path.as_os_str().to_os_string();
        name.push(".devctl.lock");
        let lock_path = PathBuf::from(name);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
            .open(&lock_path)
            .map_err(|error| format!("Failed to open run lock {}: {error}", lock_path.display()))?;
        let metadata = file.metadata().map_err(|error| {
            format!(
                "Failed to inspect run lock {}: {error}",
                lock_path.display()
            )
        })?;
        if !metadata.is_file() {
            return Err(format!(
                "Run lock is not a regular file: {}",
                lock_path.display()
            ));
        }
        match file.try_lock() {
            Ok(()) => Ok(Some(Self {
                _file: file,
                target: config_path.into(),
            })),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(error)) => Err(format!(
                "Failed to acquire run lock {}: {error}",
                lock_path.display()
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::symlink,
        process::{Child, Command},
    };

    struct Fixture(PathBuf);
    impl Fixture {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "devctl-lock-contract-{}-{name}",
                std::process::id()
            ));
            fs::create_dir(&dir).unwrap();
            fs::write(dir.join("config.toml"), "version = 1\n").unwrap();
            Self(dir)
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
    struct ChildGuard(Child);
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    fn acquire_after_close(path: &Path) -> RunLock {
        // Other concurrent tests can briefly inherit descriptors between fork and exec.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            match RunLock::acquire(path) {
                Ok(guard) => return guard,
                Err(error) => {
                    assert!(error.contains("already managed"), "{error}");
                    assert!(
                        std::time::Instant::now() < deadline,
                        "Lock stayed held: {error}"
                    );
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        }
    }

    #[test]
    fn stale_file_contents_are_ignored_and_guard_drop_releases_the_lock() {
        let fixture = Fixture::new("drop");
        let path = fixture.path();
        let sidecar = fixture.0.join("config.toml.devctl.lock");
        fs::write(&sidecar, "stale marker").unwrap();
        let guard = RunLock::acquire(&path).unwrap();
        assert!(RunLock::try_acquire(&path).unwrap().is_none());
        assert!(
            RunLock::acquire(&path)
                .err()
                .unwrap()
                .contains("already managed")
        );
        drop(guard);
        let _guard = acquire_after_close(&path);
        assert_eq!(fs::read_to_string(sidecar).unwrap(), "stale marker");
    }

    #[test]
    fn path_aliases_and_atomic_config_replacement_share_the_lock() {
        let fixture = Fixture::new("alias");
        let path = fixture.path();
        let alias = fixture.0.join("alias.toml");
        symlink(&path, &alias).unwrap();
        let _guard = RunLock::acquire(&alias).unwrap();
        assert!(RunLock::acquire(&fixture.0.join(".").join("config.toml")).is_err());
        let replacement = fixture.0.join("new.toml");
        fs::write(&replacement, "version = 2\n").unwrap();
        fs::rename(replacement, &path).unwrap();
        assert!(
            RunLock::acquire(&path)
                .err()
                .unwrap()
                .contains("already managed")
        );
    }

    #[test]
    fn invalid_lock_paths_are_errors_and_symlinks_are_not_followed() {
        let fixture = Fixture::new("errors");
        assert!(RunLock::acquire(&fixture.0.join("missing.toml")).is_err());
        let sidecar = fixture.0.join("config.toml.devctl.lock");
        fs::create_dir(&sidecar).unwrap();
        assert!(
            RunLock::acquire(&fixture.path())
                .err()
                .unwrap()
                .contains("Failed to open run lock")
        );
        fs::remove_dir(&sidecar).unwrap();
        symlink(fixture.path(), &sidecar).unwrap();
        assert!(
            RunLock::acquire(&fixture.path())
                .err()
                .unwrap()
                .contains("Failed to open run lock")
        );
        assert_eq!(fs::read_to_string(fixture.path()).unwrap(), "version = 1\n");
    }

    #[test]
    fn an_exec_child_does_not_keep_the_lock_open() {
        let fixture = Fixture::new("inheritance");
        let guard = RunLock::acquire(&fixture.path()).unwrap();
        let mut child = ChildGuard(Command::new("sleep").arg("60").spawn().unwrap());
        drop(guard);
        let _guard = acquire_after_close(&fixture.path());
        assert!(child.0.try_wait().unwrap().is_none());
    }
}
