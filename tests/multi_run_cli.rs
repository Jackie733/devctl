#![cfg(any(target_os = "linux", target_os = "macos"))]

mod support;
use support::ConfigFixture;

use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Read},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use nix::{
    errno::Errno,
    sys::signal::{Signal, kill, killpg},
    unistd::Pid,
};

fn command(path: &std::path::Path, ids: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_devctl"));
    command
        .arg("--config")
        .arg(path)
        .arg("run")
        .args(ids)
        .current_dir(std::env::temp_dir());
    command
}

fn reader(pipe: impl Read + Send + 'static) -> (Receiver<String>, JoinHandle<()>) {
    let (sender, receiver) = mpsc::channel();
    let handle = thread::spawn(move || {
        for line in BufReader::new(pipe).lines() {
            if sender.send(line.unwrap()).is_err() {
                break;
            }
        }
    });
    (receiver, handle)
}

struct Batch {
    child: Child,
    pids: BTreeMap<String, Pid>,
    worker: Option<Pid>,
    stdout: Receiver<String>,
    stderr: Receiver<String>,
    readers: Vec<JoinHandle<()>>,
    _config: ConfigFixture,
}

impl Batch {
    fn spawn(ids: &[&str]) -> Self {
        let config = ConfigFixture::new("process.toml");
        let mut child = command(&config.path, ids)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let (stdout, out_reader) = reader(child.stdout.take().unwrap());
        let (stderr, err_reader) = reader(child.stderr.take().unwrap());
        Self {
            child,
            pids: BTreeMap::new(),
            worker: None,
            stdout,
            stderr,
            readers: vec![out_reader, err_reader],
            _config: config,
        }
    }

    fn ready(&mut self, ids: &[&str], needs_worker: bool, needs_ready_line: bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut ready_line = !needs_ready_line;
        while self.pids.len() < ids.len() || (needs_worker && self.worker.is_none()) || !ready_line
        {
            let line = self
                .stdout
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("Every selected running service must report startup.");
            for id in ids {
                if let Some(value) = line.strip_prefix(&format!("Started service '{id}' with PID "))
                {
                    let pid = Pid::from_raw(value.trim_end_matches('.').parse().unwrap());
                    assert!(
                        self.pids.insert((*id).into(), pid).is_none(),
                        "Duplicate process: {id}"
                    );
                }
            }
            if let Some(value) = line.strip_prefix("Worker PID: ") {
                self.worker = Some(Pid::from_raw(value.parse().unwrap()));
            }
            ready_line |= line == "Ready";
        }
        for pid in self.pids.values() {
            assert_eq!(kill(*pid, None), Ok(()));
        }
    }

    fn stop(&mut self, signal: Signal, expected_code: i32) {
        kill(
            Pid::from_raw(i32::try_from(self.child.id()).unwrap()),
            signal,
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline, "The batch did not stop.");
            thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(status.code(), Some(expected_code));
        for (id, pid) in &self.pids {
            assert_eq!(
                kill(*pid, None),
                Err(Errno::ESRCH),
                "Child not reaped: {id}"
            );
        }
        self.pids.clear();
        if let Some(pid) = self.worker {
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                if !support::process_is_live(pid) {
                    break;
                }
                assert!(Instant::now() < deadline, "The worker is still running.");
                thread::sleep(Duration::from_millis(10));
            }
            self.worker = None;
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        while self.readers.iter().any(|reader| !reader.is_finished()) {
            assert!(Instant::now() < deadline, "Output pipes remained open.");
            thread::sleep(Duration::from_millis(10));
        }
        for reader in self.readers.drain(..) {
            reader.join().unwrap();
        }
    }
}

impl Drop for Batch {
    fn drop(&mut self) {
        for pid in self.pids.values() {
            let _ = killpg(*pid, Signal::SIGKILL);
        }
        if let Some(pid) = self.worker {
            let _ = kill(pid, Signal::SIGKILL);
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn run_requires_targets_or_all_and_help_does_not_load_config() {
    let fixture = ConfigFixture::new("process.toml");
    let output = command(&fixture.path, &[]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let output = command(
        &fixture.path.with_file_name("missing-config.toml"),
        &["--help"],
    )
    .output()
    .unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("[SERVICE]...")
    );
}

#[test]
fn invalid_batch_selection_is_rejected_before_any_spawn() {
    for (example, ids, expected) in [
        (
            "process.toml",
            vec!["long_running", "unknown"],
            "Unknown service 'unknown'.",
        ),
        (
            "dependencies/chain.toml",
            vec!["db", "web"],
            "Service 'web' has dependencies.",
        ),
    ] {
        let fixture = ConfigFixture::new(example);
        let output = command(&fixture.path, &ids).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr).unwrap().contains(expected));
    }
}

#[test]
fn repeated_ids_run_once_and_every_natural_exit_is_reported() {
    let fixture = ConfigFixture::new("process.toml");
    let output = command(&fixture.path, &["hello", "hello", "failure"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.matches("Started service 'hello' with PID ").count(),
        1
    );
    assert_eq!(
        stdout
            .matches("Started service 'failure' with PID ")
            .count(),
        1
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("Service 'hello' exited unexpectedly (exit status: 0)."));
    assert!(stderr.contains("Service 'failure' exited unexpectedly (exit status: 7)."));
}

#[test]
fn three_services_start_together_and_both_terminal_signals_clean_up_the_batch() {
    for signal in [Signal::SIGINT, Signal::SIGTERM] {
        let ids = ["long_running", "process_tree", "graceful"];
        let mut batch = Batch::spawn(&ids);
        batch.ready(&ids, true, true);
        assert!(batch.child.try_wait().unwrap().is_none());
        batch.stop(signal, 0);
    }
}

#[test]
fn failures_are_reported_immediately_while_other_services_remain_running() {
    for failed in ["missing", "hello", "failure", "signal"] {
        let mut batch = Batch::spawn(&[failed, "long_running"]);
        batch.ready(&["long_running"], false, false);
        let line = batch.stderr.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(line.contains(&format!("'{failed}'")), "{line}");
        assert!(batch.child.try_wait().unwrap().is_none());
        assert_eq!(kill(batch.pids["long_running"], None), Ok(()));
        batch.stop(Signal::SIGINT, 1);
        let stderr: Vec<_> = batch.stderr.try_iter().collect();
        assert!(
            stderr
                .iter()
                .any(|line| line.contains("One or more services failed."))
        );
    }
}

#[test]
fn all_selects_only_autostart_services_and_rejects_explicit_targets() {
    let fixture = ConfigFixture::new("process.toml");
    let output = command(&fixture.path, &["--all"]).output().unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "No services selected."
    );
    let output = command(&fixture.path, &["--all", "hello"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));

    let mut config: toml::Value =
        toml::from_str(&std::fs::read_to_string(&fixture.path).unwrap()).unwrap();
    for id in ["hello", "failure"] {
        config["services"][id]["autostart"] = toml::Value::Boolean(true);
    }
    std::fs::write(&fixture.path, toml::to_string(&config).unwrap()).unwrap();
    let output = command(&fixture.path, &["--all"]).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.matches("Started service '").count(), 2);
    assert!(stdout.contains("Started service 'hello'"));
    assert!(stdout.contains("Started service 'failure'"));
}
