use std::{
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use super::{manifest::Directory, safe};

pub(super) struct Limits {
    pub depth: usize,
    pub entries: usize,
    pub manifests: usize,
    pub declaration_bytes: usize,
    pub lock_bytes: usize,
    pub total_bytes: usize,
    pub elapsed: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            depth: 6,
            entries: 10_000,
            manifests: 200,
            declaration_bytes: 1024 * 1024,
            lock_bytes: 16 * 1024 * 1024,
            total_bytes: 64 * 1024 * 1024,
            elapsed: Duration::from_secs(5),
        }
    }
}

pub(super) struct Reader {
    pub limits: Limits,
    started: Instant,
    entries: usize,
    manifests: usize,
    bytes: usize,
}

impl Reader {
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            started: Instant::now(),
            entries: 0,
            manifests: 0,
            bytes: 0,
        }
    }

    pub fn check(&self) -> Result<(), String> {
        if self.started.elapsed() >= self.limits.elapsed {
            return Err(
                "Discovery incomplete: time limit reached. Select a narrower directory.".into(),
            );
        }
        Ok(())
    }

    pub fn ancestors(&self, root: &Path) -> Result<Vec<PathBuf>, String> {
        let mut ancestors = Vec::new();
        for path in root.ancestors().take(17) {
            self.check()?;
            if ancestors.len() == 16 {
                return Err("Discovery incomplete: parent inspection limit reached. Select the enclosing repository root.".into());
            }
            ancestors.push(path.into());
            match fs::symlink_metadata(path.join(".git")) {
                Ok(metadata) if metadata.is_dir() || metadata.is_file() => break,
                Ok(_) => {
                    return Err(format!(
                        "Unsupported Git boundary marker in {}",
                        safe(path.display())
                    ));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(format!(
                        "Cannot inspect repository boundary {}: {error}",
                        safe(path.display())
                    ));
                }
            }
        }
        Ok(ancestors)
    }

    pub fn directory(&mut self, path: &Path, boundary: &Path) -> Result<Directory, String> {
        super::manifest::read(self, path, boundary)
    }

    /// Open only regular, contained declarations. NONBLOCK also covers FIFO races.
    pub fn read(
        &mut self,
        path: &Path,
        boundary: &Path,
        lock: bool,
    ) -> Result<Option<String>, String> {
        self.check()?;
        match fs::symlink_metadata(path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("Cannot inspect {}: {error}", safe(path.display()))),
        }
        let resolved = path.canonicalize().map_err(|error| {
            format!(
                "Cannot resolve declaration {}: {error}",
                safe(path.display())
            )
        })?;
        if !resolved.starts_with(boundary) {
            return Err(format!(
                "Declaration {} escapes repository boundary {}",
                safe(path.display()),
                safe(boundary.display())
            ));
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
            .open(&resolved)
            .map_err(|error| format!("Cannot read {}: {error}", safe(path.display())))?;
        let metadata = file
            .metadata()
            .map_err(|error| format!("Cannot inspect {}: {error}", safe(path.display())))?;
        if !metadata.is_file() {
            return Err(format!(
                "Declaration is not a regular file: {}",
                safe(path.display())
            ));
        }
        if path.file_name().is_some_and(|name| name == "package.json") {
            self.manifests += 1;
            if self.manifests > self.limits.manifests {
                return Err("Discovery incomplete: manifest count limit reached. Select a narrower directory.".into());
            }
        }
        let cap = if lock {
            self.limits.lock_bytes
        } else {
            self.limits.declaration_bytes
        };
        let remaining = self.limits.total_bytes.saturating_sub(self.bytes);
        if metadata.len() > cap.min(remaining) as u64 {
            return Err(format!(
                "Discovery incomplete: input byte limit reached at {}",
                safe(path.display())
            ));
        }
        let mut bytes = Vec::new();
        file.take(cap.min(remaining) as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("Cannot read {}: {error}", safe(path.display())))?;
        if bytes.len() > cap.min(remaining) {
            return Err(format!(
                "Discovery incomplete: input byte limit reached at {}",
                safe(path.display())
            ));
        }
        self.bytes += bytes.len();
        self.check()?;
        String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| format!("Declaration is not UTF-8: {}", safe(path.display())))
    }

    pub fn children(&mut self, path: &Path) -> Result<Vec<PathBuf>, String> {
        const EXCLUDED: &[&str] = &[
            ".git",
            "node_modules",
            ".pnpm",
            ".yarn",
            "vendor",
            "target",
            "dist",
            "build",
            "coverage",
            ".next",
            ".turbo",
        ];
        self.check()?;
        let mut children = Vec::new();
        let entries = fs::read_dir(path).map_err(|error| {
            format!("Cannot inspect directory {}: {error}", safe(path.display()))
        })?;
        for entry in entries {
            self.check()?;
            self.entries += 1;
            if self.entries > self.limits.entries {
                return Err("Discovery incomplete: directory entry limit reached. Select a narrower directory.".into());
            }
            let entry = entry.map_err(|error| {
                format!("Cannot inspect directory {}: {error}", safe(path.display()))
            })?;
            if EXCLUDED.iter().any(|name| entry.file_name() == *name) {
                continue;
            }
            let kind = entry.file_type().map_err(|error| {
                format!("Cannot inspect {}: {error}", safe(entry.path().display()))
            })?;
            if kind.is_dir() {
                children.push(entry.path());
            }
        }
        children.sort();
        Ok(children)
    }
}
