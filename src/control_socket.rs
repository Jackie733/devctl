use std::{
    fs::{self, DirBuilder},
    io::ErrorKind,
    os::unix::{
        fs::{DirBuilderExt, FileTypeExt, MetadataExt},
        net::UnixListener as StdUnixListener,
    },
    path::{Path, PathBuf},
};

use nix::unistd::geteuid;
use tokio::net::{UnixListener, UnixStream};

use crate::run_lock::RunLock;

pub struct ControlSocket<'a> {
    listener: UnixListener,
    path: PathBuf,
    identity: (u64, u64),
    _lock: &'a RunLock,
}

impl<'a> ControlSocket<'a> {
    pub async fn accept(&self) -> std::io::Result<UnixStream> {
        let (stream, _) = self.listener.accept().await?;
        Ok(stream)
    }

    // The caller must hold the lock for this canonical configuration path.
    pub fn bind(config_path: &Path, lock: &'a RunLock) -> Result<Self, String> {
        let mut name = config_path.as_os_str().to_os_string();
        name.push(".devctl");
        let directory = PathBuf::from(name);

        match DirBuilder::new().mode(0o700).create(&directory) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(format!(
                    "Failed to create runtime directory {}: {error}",
                    directory.display()
                ));
            }
        }

        let metadata = fs::symlink_metadata(&directory).map_err(|error| {
            format!(
                "Failed to inspect runtime directory {}: {error}",
                directory.display()
            )
        })?;
        if !metadata.is_dir()
            || metadata.uid() != geteuid().as_raw()
            || metadata.mode() & 0o777 != 0o700
        {
            return Err(format!(
                "Runtime directory must be an owned directory with mode 0700: {}",
                directory.display()
            ));
        }

        let path = directory.join("control.sock");
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_socket() => {
                fs::remove_file(&path).map_err(|error| {
                    format!(
                        "Failed to remove stale control socket {}: {error}",
                        path.display()
                    )
                })?;
            }
            Ok(_) => {
                return Err(format!(
                    "Refusing to replace non-socket path: {}",
                    path.display()
                ));
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "Failed to inspect control socket {}: {error}",
                    path.display()
                ));
            }
        }

        let listener = StdUnixListener::bind(&path).map_err(|error| {
            format!("Failed to bind control socket {}: {error}", path.display())
        })?;
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            format!(
                "Failed to inspect control socket {}: {error}",
                path.display()
            )
        })?;

        listener
            .set_nonblocking(true)
            .map_err(|error| format!("Failed to make control socket nonblocking: {error}"))?;
        let listener = UnixListener::from_std(listener)
            .map_err(|error| format!("Failed to register control socket: {error}"))?;

        Ok(Self {
            listener,
            path,
            identity: (metadata.dev(), metadata.ino()),
            _lock: lock,
        })
    }
}

impl Drop for ControlSocket<'_> {
    fn drop(&mut self) {
        if let Ok(metadata) = fs::symlink_metadata(&self.path)
            && metadata.file_type().is_socket()
            && (metadata.dev(), metadata.ino()) == self.identity
        {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::Permissions,
        os::unix::{
            fs::{PermissionsExt, symlink},
            net::UnixStream,
        },
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!(
                "devctl-socket-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&dir).unwrap();
            fs::write(dir.join("config.toml"), "test").unwrap();
            Self(dir)
        }
        fn config(&self) -> PathBuf {
            self.0.join("config.toml")
        }
        fn runtime(&self) -> PathBuf {
            self.0.join("config.toml.devctl")
        }
        fn socket(&self) -> PathBuf {
            self.runtime().join("control.sock")
        }
        fn create_runtime(&self) {
            DirBuilder::new()
                .mode(0o700)
                .create(self.runtime())
                .unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[tokio::test]
    async fn binds_a_connectable_socket_in_a_private_directory_and_cleans_up() {
        let fixture = Fixture::new();
        let lock = RunLock::acquire(&fixture.config()).unwrap();
        let socket = ControlSocket::bind(&fixture.config(), &lock).unwrap();
        assert_eq!(
            fs::metadata(fixture.runtime()).unwrap().mode() & 0o777,
            0o700
        );
        let connection = UnixStream::connect(fixture.socket()).unwrap();
        drop(connection);
        drop(socket);
        assert!(!fixture.socket().exists());
        assert!(fixture.runtime().is_dir());
        assert!(RunLock::acquire(&fixture.config()).is_err());
    }

    #[tokio::test]
    async fn stale_sockets_are_replaced_after_acquiring_the_config_lock() {
        let fixture = Fixture::new();
        fixture.create_runtime();
        drop(StdUnixListener::bind(fixture.socket()).unwrap());
        assert!(UnixStream::connect(fixture.socket()).is_err());
        let lock = RunLock::acquire(&fixture.config()).unwrap();
        let _socket = ControlSocket::bind(&fixture.config(), &lock).unwrap();
        assert!(UnixStream::connect(fixture.socket()).is_ok());
    }

    #[tokio::test]
    async fn refuses_files_directories_and_symlinks_at_the_socket_path() {
        for kind in ["file", "directory", "symlink"] {
            let fixture = Fixture::new();
            fixture.create_runtime();
            match kind {
                "file" => fs::write(fixture.socket(), "keep me").unwrap(),
                "directory" => fs::create_dir(fixture.socket()).unwrap(),
                _ => symlink(fixture.config(), fixture.socket()).unwrap(),
            }
            let lock = RunLock::acquire(&fixture.config()).unwrap();
            assert!(
                ControlSocket::bind(&fixture.config(), &lock)
                    .err()
                    .unwrap()
                    .contains("non-socket")
            );
            assert!(fs::symlink_metadata(fixture.socket()).is_ok());
            assert_eq!(fs::read_to_string(fixture.config()).unwrap(), "test");
            if kind == "file" {
                assert_eq!(fs::read_to_string(fixture.socket()).unwrap(), "keep me");
            }
        }
    }

    #[tokio::test]
    async fn refuses_unsafe_runtime_directories_without_changing_permissions_or_targets() {
        for kind in ["public", "file", "symlink"] {
            let fixture = Fixture::new();
            match kind {
                "public" => {
                    fixture.create_runtime();
                    fs::set_permissions(fixture.runtime(), Permissions::from_mode(0o755)).unwrap();
                }
                "file" => fs::write(fixture.runtime(), "keep me").unwrap(),
                _ => symlink(&fixture.0, fixture.runtime()).unwrap(),
            }
            let lock = RunLock::acquire(&fixture.config()).unwrap();
            assert!(
                ControlSocket::bind(&fixture.config(), &lock)
                    .err()
                    .unwrap()
                    .contains("0700")
            );
            assert!(!fixture.0.join("control.sock").exists());
            if kind == "public" {
                assert_eq!(
                    fs::metadata(fixture.runtime()).unwrap().mode() & 0o777,
                    0o755
                );
            }
        }
    }

    #[tokio::test]
    async fn drop_keeps_a_replacement_path_and_bind_errors_are_reported() {
        let fixture = Fixture::new();
        let lock = RunLock::acquire(&fixture.config()).unwrap();
        let socket = ControlSocket::bind(&fixture.config(), &lock).unwrap();
        fs::remove_file(fixture.socket()).unwrap();
        fs::write(fixture.socket(), "replacement").unwrap();
        drop(socket);
        assert_eq!(fs::read_to_string(fixture.socket()).unwrap(), "replacement");

        let config = fixture.0.join("a".repeat(150));
        fs::write(&config, "test").unwrap();
        let lock = RunLock::acquire(&config).unwrap();
        assert!(
            ControlSocket::bind(&config, &lock)
                .err()
                .unwrap()
                .contains("Failed to bind")
        );
    }
}
