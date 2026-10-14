#![cfg(target_os = "linux")]

mod support;
use support::ConfigFixture;

use nix::{
    sys::signal::{Signal, kill},
    unistd::Pid,
};
use std::{
    fs::{self, OpenOptions, TryLockError},
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn command(path: &Path, operation: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_devctl"));
    command
        .arg("--config")
        .arg(path)
        .arg(operation)
        .current_dir(std::env::temp_dir());
    command
}

struct Owner(Child);
impl Owner {
    fn launch(path: &Path) -> Self {
        Self(
            command(path, "supervise")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        )
    }

    fn wait_for_lock(&mut self, path: &Path) {
        let mut name = path.as_os_str().to_os_string();
        name.push(".devctl.lock");
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            assert!(
                self.0.try_wait().unwrap().is_none(),
                "Supervisor exited before taking the lock."
            );
            if let Ok(file) = OpenOptions::new().read(true).write(true).open(&name) {
                match file.try_lock() {
                    Err(TryLockError::WouldBlock) => return,
                    Ok(()) => {}
                    Err(error) => panic!("Unexpected lock error: {error}"),
                }
            }
            assert!(Instant::now() < deadline, "Supervisor never took the lock.");
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn stop(&mut self, signal: Signal) {
        kill(Pid::from_raw(self.0.id().try_into().unwrap()), signal).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                assert!(status.success());
                return;
            }
            assert!(Instant::now() < deadline, "Supervisor did not stop.");
            thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn idle_supervisor_never_autostarts_holds_the_lock_and_releases_on_both_signals() {
    for signal in [Signal::SIGINT, Signal::SIGTERM] {
        let fixture = ConfigFixture::new("process.toml");
        let marker = fixture.path.with_file_name("unexpected-autostart");
        let mut config: toml::Value =
            toml::from_str(&fs::read_to_string(&fixture.path).unwrap()).unwrap();
        config["services"]["hello"]["autostart"] = toml::Value::Boolean(true);
        config["services"]["hello"]["cmd"] = toml::Value::Array(
            [
                "sh",
                "-c",
                "printf auto > \"$1\"",
                "sh",
                marker.to_str().unwrap(),
            ]
            .into_iter()
            .map(|value| toml::Value::String(value.into()))
            .collect(),
        );
        fs::write(&fixture.path, toml::to_string(&config).unwrap()).unwrap();
        let mut owner = Owner::launch(&fixture.path);
        owner.wait_for_lock(&fixture.path);
        for operation in ["validate", "plan"] {
            assert!(
                command(&fixture.path, operation)
                    .output()
                    .unwrap()
                    .status
                    .success()
            );
        }
        let output = command(&fixture.path, "supervise").output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("already managed")
        );
        let output = command(&fixture.path, "run").arg("hello").output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("already managed")
        );
        assert!(!marker.exists());
        owner.stop(signal);
        assert!(!marker.exists());
        let mut successor = Owner::launch(&fixture.path);
        successor.wait_for_lock(&fixture.path);
        successor.stop(signal);
        assert!(!marker.exists());
    }
}

#[test]
fn internal_entry_is_hidden_and_invalid_configuration_fails_before_locking() {
    let fixture = ConfigFixture::new("process.toml");
    let output = command(&fixture.path, "--help").output().unwrap();
    assert!(output.status.success());
    assert!(
        !String::from_utf8(output.stdout)
            .unwrap()
            .contains("supervise")
    );
    fs::write(&fixture.path, "not valid toml").unwrap();
    let output = command(&fixture.path, "supervise").output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("Failed to parse")
    );
    assert!(
        !fixture
            .path
            .with_file_name("devctl.toml.devctl.lock")
            .exists()
    );
}

fn socket_path(config: &Path) -> std::path::PathBuf {
    let mut name = config.as_os_str().to_os_string();
    name.push(".devctl");
    std::path::PathBuf::from(name).join("control.sock")
}

fn wait_for_socket(owner: &mut Owner, path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(owner.0.try_wait().unwrap().is_none());
        if std::os::unix::net::UnixStream::connect(path).is_ok() {
            return;
        }
        assert!(Instant::now() < deadline, "No bound control socket.");
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn supervisor_binds_before_waiting_preserves_live_endpoint_and_cleans_up() {
    use std::os::unix::fs::MetadataExt;
    for signal in [Signal::SIGINT, Signal::SIGTERM] {
        let fixture = ConfigFixture::new("process.toml");
        let path = socket_path(&fixture.path);
        let mut owner = Owner::launch(&fixture.path);
        wait_for_socket(&mut owner, &path);
        let inode = fs::symlink_metadata(&path).unwrap().ino();
        let output = command(&fixture.path, "supervise").output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("already managed")
        );
        assert_eq!(fs::symlink_metadata(&path).unwrap().ino(), inode);
        assert!(std::os::unix::net::UnixStream::connect(&path).is_ok());
        owner.stop(signal);
        assert!(!path.exists());
    }
}

#[test]
fn crash_leaves_a_socket_that_the_next_lock_holder_can_replace() {
    let fixture = ConfigFixture::new("process.toml");
    let path = socket_path(&fixture.path);
    let mut owner = Owner::launch(&fixture.path);
    wait_for_socket(&mut owner, &path);
    owner.0.kill().unwrap();
    owner.0.wait().unwrap();
    assert!(path.exists());
    assert!(std::os::unix::net::UnixStream::connect(&path).is_err());
    let mut successor = Owner::launch(&fixture.path);
    wait_for_socket(&mut successor, &path);
    successor.stop(Signal::SIGTERM);
    assert!(!path.exists());
}
