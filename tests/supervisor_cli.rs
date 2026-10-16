#![cfg(any(target_os = "linux", target_os = "macos"))]

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

fn connect_probe(path: &Path) -> std::os::unix::net::UnixStream {
    let stream = std::os::unix::net::UnixStream::connect(path).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
}

fn read_probe(mut stream: std::os::unix::net::UnixStream, config: &Path) {
    use std::{
        io::{Read, Write},
        os::unix::ffi::OsStrExt,
    };
    stream.write_all(b"DEVCTL 1\n").unwrap();
    let mut header = [0u8; 9];
    stream.read_exact(&mut header).unwrap();
    assert_eq!(&header, b"DEVCTL 1\n");
    let mut length = [0u8; 4];
    stream.read_exact(&mut length).unwrap();
    let length = u32::from_be_bytes(length) as usize;
    assert!(length <= 16 * 1024);
    let mut identity = vec![0; length];
    stream.read_exact(&mut identity).unwrap();
    assert_eq!(
        identity,
        config.canonicalize().unwrap().as_os_str().as_bytes()
    );
    let mut extra = [0];
    assert_eq!(stream.read(&mut extra).unwrap(), 0);
}

#[test]
fn probe_returns_canonical_identity_through_a_config_alias() {
    let fixture = ConfigFixture::new("process.toml");
    let alias = fixture.path.with_file_name("alias.toml");
    std::os::unix::fs::symlink(&fixture.path, &alias).unwrap();
    let path = socket_path(&fixture.path);
    let mut owner = Owner::launch(&alias);
    wait_for_socket(&mut owner, &path);
    read_probe(connect_probe(&path), &fixture.path);
    owner.stop(Signal::SIGTERM);
}

#[test]
fn malformed_clients_do_not_prevent_other_clients_from_receiving_a_probe() {
    use std::io::{Read, Write};
    let fixture = ConfigFixture::new("process.toml");
    let path = socket_path(&fixture.path);
    let mut owner = Owner::launch(&fixture.path);
    wait_for_socket(&mut owner, &path);
    let mut silent = connect_probe(&path);
    let mut invalid = connect_probe(&path);
    invalid.write_all(b"DEVCTL 2\n").unwrap();
    let mut reply = [0];
    assert_eq!(invalid.read(&mut reply).unwrap(), 0);
    read_probe(connect_probe(&path), &fixture.path);
    assert!(owner.0.try_wait().unwrap().is_none());
    owner.stop(Signal::SIGINT);
    assert!(!path.exists());
    assert_eq!(silent.read(&mut reply).unwrap(), 0);
}

#[test]
fn timeouts_release_capacity_and_signals_do_not_wait_for_silent_clients() {
    let fixture = ConfigFixture::new("process.toml");
    let path = socket_path(&fixture.path);
    let mut owner = Owner::launch(&fixture.path);
    wait_for_socket(&mut owner, &path);
    let mut silent = Vec::new();
    for _ in 0..16 {
        silent.push(connect_probe(&path));
    }
    // This request must succeed after silent clients time out and slots are reclaimed.
    read_probe(connect_probe(&path), &fixture.path);
    silent.clear();
    for _ in 0..16 {
        silent.push(connect_probe(&path));
    }
    let before = Instant::now();
    owner.stop(Signal::SIGTERM);
    assert!(
        before.elapsed() < Duration::from_millis(1500),
        "Signal shutdown waited for probe deadlines."
    );
    assert!(!path.exists());
}

#[cfg(target_os = "macos")]
#[test]
fn detached_supervisor_does_not_hold_an_inherited_caller_pipe_open() {
    let fixture = ConfigFixture::new("process.toml");
    let output = Command::new("python3")
        .args([
            "-c",
            r#"
import os, select, signal, subprocess, sys
signal.alarm(15)
reader, writer = os.pipe()
process = subprocess.Popen([sys.argv[1], '--config', sys.argv[2], 'supervise', '--detached'],
                           pass_fds=(writer,), stdout=subprocess.PIPE, stderr=subprocess.PIPE)
os.close(writer)
try:
    assert process.stdout.readline() == b'READY\n'
    assert process.poll() is None
    assert select.select([reader], [], [], 2)[0], 'Supervisor retained caller pipe'
    assert os.read(reader, 1) == b''
finally:
    process.terminate()
    process.communicate(timeout=5)
    os.close(reader)
assert process.returncode == 0
"#,
        ])
        .arg(env!("CARGO_BIN_EXE_devctl"))
        .arg(&fixture.path)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}
