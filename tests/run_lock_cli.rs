#![cfg(any(target_os = "linux", target_os = "macos"))]

mod support;
use support::ConfigFixture;

use nix::{
    errno::Errno,
    sys::signal::{Signal, kill, killpg},
    unistd::Pid,
};
use std::{
    io::{BufRead, BufReader},
    os::unix::fs::symlink,
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

fn command(path: &Path, service: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_devctl"));
    command
        .arg("--config")
        .arg(path)
        .args(["run", service])
        .current_dir(support::temp_root());
    command
}

struct Owner {
    child: Child,
    service: String,
    pid: Option<Pid>,
    ready: bool,
    lines: Receiver<String>,
    reader: Option<JoinHandle<()>>,
}
impl Owner {
    fn launch(path: &Path, service: &str) -> Self {
        let mut child = command(path, service)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let pipe = child.stdout.take().unwrap();
        let (sender, lines) = mpsc::channel();
        let reader = thread::spawn(move || {
            for line in BufReader::new(pipe).lines() {
                if sender.send(line.unwrap()).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            service: service.into(),
            pid: None,
            ready: false,
            lines,
            reader: Some(reader),
        }
    }
    fn observe(&mut self) {
        while let Ok(line) = self.lines.try_recv() {
            self.record(&line);
        }
    }
    fn record(&mut self, line: &str) {
        if let Some(value) =
            line.strip_prefix(&format!("Started service '{}' with PID ", self.service))
        {
            assert!(self.pid.is_none());
            self.pid = Some(Pid::from_raw(value.trim_end_matches('.').parse().unwrap()));
        }
        self.ready |= line == "Ready";
    }
    fn wait_ready(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.pid.is_none() || (self.service == "ignore_term" && !self.ready) {
            let line = self
                .lines
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap();
            self.record(&line);
        }
    }
    fn signal(&self) {
        kill(
            Pid::from_raw(i32::try_from(self.child.id()).unwrap()),
            Signal::SIGTERM,
        )
        .unwrap();
    }
    fn finish(&mut self, code: i32) {
        let deadline = Instant::now() + Duration::from_secs(10);
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline, "Owner did not exit.");
            thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(status.code(), Some(code));
        if let Some(pid) = self.pid.take() {
            assert_eq!(kill(pid, None), Err(Errno::ESRCH));
        }
        let reader = self.reader.take().unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !reader.is_finished() {
            assert!(Instant::now() < deadline, "Output remained open.");
            thread::sleep(Duration::from_millis(10));
        }
        reader.join().unwrap();
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.observe();
        if let Some(pid) = self.pid {
            let _ = killpg(pid, Signal::SIGKILL);
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn assert_conflict(path: &Path) {
    let output = command(path, "hello").output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("already managed")
    );
}

#[test]
fn aliases_read_only_commands_and_shutdown_obey_the_config_lock() {
    let fixture = ConfigFixture::new("process.toml");
    let mut owner = Owner::launch(&fixture.path, "ignore_term");
    owner.wait_ready();
    let alias = fixture.path.with_file_name("alias.toml");
    symlink(&fixture.path, &alias).unwrap();
    assert_conflict(&fixture.path);
    assert_conflict(&alias);
    assert_conflict(fixture.path.strip_prefix(support::temp_root()).unwrap());
    for operation in ["validate", "plan"] {
        let output = Command::new(env!("CARGO_BIN_EXE_devctl"))
            .arg("--config")
            .arg(&fixture.path)
            .arg(operation)
            .output()
            .unwrap();
        assert!(output.status.success());
    }
    owner.signal();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let line = owner
            .lines
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap();
        if line == "Stopping service 'ignore_term'." {
            break;
        }
    }
    assert_conflict(&fixture.path);
    assert!(owner.child.try_wait().unwrap().is_none());
    owner.finish(0);
    assert!(
        fixture
            .path
            .with_file_name("devctl.toml.devctl.lock")
            .is_file()
    );
    let output = command(&fixture.path, "hello").output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("Started service 'hello'")
    );
}

#[test]
fn distinct_configs_can_be_owned_concurrently() {
    let a = ConfigFixture::new("process.toml");
    let b = ConfigFixture::new("process.toml");
    let mut first = Owner::launch(&a.path, "long_running");
    let mut second = Owner::launch(&b.path, "long_running");
    first.wait_ready();
    second.wait_ready();
    assert!(first.child.try_wait().unwrap().is_none());
    assert!(second.child.try_wait().unwrap().is_none());
    first.signal();
    second.signal();
    first.finish(0);
    second.finish(0);
}

#[test]
fn concurrent_launches_have_exactly_one_winner() {
    let fixture = ConfigFixture::new("process.toml");
    let mut a = Owner::launch(&fixture.path, "long_running");
    let mut b = Owner::launch(&fixture.path, "long_running");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        a.observe();
        b.observe();
        let a_done = a.child.try_wait().unwrap();
        let b_done = b.child.try_wait().unwrap();
        if a.pid.is_some() && b_done.is_some() {
            assert!(b.pid.is_none());
            b.finish(1);
            a.signal();
            a.finish(0);
            break;
        }
        if b.pid.is_some() && a_done.is_some() {
            assert!(a.pid.is_none());
            a.finish(1);
            b.signal();
            b.finish(0);
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Concurrent ownership did not resolve."
        );
        thread::sleep(Duration::from_millis(10));
    }
}
