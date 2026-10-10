#![cfg(unix)]

use std::{
    io::{BufRead, BufReader},
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use nix::{
    errno::Errno,
    sys::signal::{Signal, kill, killpg},
    unistd::Pid,
};

fn run_example(example: &str, service: &str) -> std::process::Output {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(example);

    Command::new(env!("CARGO_BIN_EXE_devctl"))
        .arg("--config")
        .arg(path)
        .args(["run", service])
        .current_dir(std::env::temp_dir())
        .output()
        .expect("The devctl binary must run.")
}

fn assert_unexpected_exit(service: &str, status: &str) -> String {
    let output = run_example("process.toml", service);
    assert_eq!(output.status.code(), Some(1), "{output:?}");

    let stdout = String::from_utf8(output.stdout).expect("Output must be UTF-8.");
    let stderr = String::from_utf8(output.stderr).expect("Errors must be UTF-8.");
    assert!(stdout.contains(&format!("Started service '{service}' with PID ")));
    assert!(stdout.contains(&format!("Service '{service}' exited: {status}")));
    assert!(stderr.contains(&format!(
        "Service '{service}' exited unexpectedly ({status}"
    )));
    stdout
}

#[test]
fn successful_command_is_an_unexpected_service_exit() {
    let stdout = assert_unexpected_exit("hello", "exit status: 0");
    assert!(stdout.contains("Hello from child\n"));
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .unwrap();
    assert!(stdout.contains(&format!("Child cwd: {}\n", repo.display())));
}

#[test]
fn nonzero_child_exit_is_reported() {
    assert_unexpected_exit("failure", "exit status: 7");
}

#[test]
fn arguments_keep_boundaries_and_are_not_shell_expanded() {
    let stdout = assert_unexpected_exit("arguments", "exit status: 0");
    assert!(stdout.contains("two words\n$HOME\n*.rs\n"));
}

#[test]
fn explicit_working_directory_is_applied_to_child() {
    let stdout = assert_unexpected_exit("cwd", "exit status: 0");
    let cwd = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .canonicalize()
        .unwrap();
    assert!(stdout.lines().any(|line| line == cwd.to_str().unwrap()));
}

#[test]
fn child_stdin_receives_eof() {
    let stdout = assert_unexpected_exit("stdin_eof", "exit status: 0");
    assert!(stdout.contains("stdin EOF\n"));
}

#[test]
fn signal_termination_is_reported() {
    assert_unexpected_exit("signal", "signal: 15");
}

#[test]
fn missing_executable_is_a_spawn_error() {
    let output = run_example("process.toml", "missing");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("Errors must be UTF-8.");
    assert!(stderr.contains("Failed to start service 'missing':"));
    assert!(!stderr.contains("exited unexpectedly"));
}

#[test]
fn unknown_service_is_rejected_before_spawn() {
    let output = run_example("process.toml", "unknown");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("Errors must be UTF-8.");
    assert!(stderr.contains("Unknown service 'unknown'."));
}

#[test]
fn dependencies_are_rejected_before_spawn() {
    let output = run_example("dependencies/chain.toml", "web");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("Errors must be UTF-8.");
    assert!(stderr.contains("Dependency execution is not supported yet."));
}

// Keep failed signal tests from leaving either process running.
struct RunningService {
    devctl: Child,
    service_pid: Option<Pid>,
    worker_pid: Option<Pid>,
}

impl Drop for RunningService {
    fn drop(&mut self) {
        if let Some(pid) = self.service_pid {
            let _ = killpg(pid, Signal::SIGKILL);
        }
        if let Some(pid) = self.worker_pid {
            let _ = kill(pid, Signal::SIGKILL);
        }
        let _ = self.devctl.kill();
        let _ = self.devctl.wait();
    }
}

fn assert_service_exit(
    signal: Option<Signal>,
    service: &str,
    has_worker: bool,
    ready_line: Option<&str>,
) -> (Vec<String>, Duration) {
    let config = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/process.toml");
    let devctl = Command::new(env!("CARGO_BIN_EXE_devctl"))
        .arg("--config")
        .arg(config)
        .args(["run", service])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("devctl must start.");
    let mut running = RunningService {
        devctl,
        service_pid: None,
        worker_pid: None,
    };

    let stdout = running.devctl.stdout.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if sender.send(line.unwrap()).is_err() {
                break;
            }
        }
    });

    // Synchronize on spawn, rather than sleeping and hoping the handler is ready.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut ready = ready_line.is_none();
    while running.service_pid.is_none() || (has_worker && running.worker_pid.is_none()) || !ready {
        let line = receiver
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("devctl and the worker must report startup within five seconds.");
        if let Some(value) = line.strip_prefix(&format!("Started service '{service}' with PID ")) {
            running.service_pid = Some(Pid::from_raw(value.trim_end_matches('.').parse().unwrap()));
        } else if let Some(value) = line.strip_prefix("Worker PID: ") {
            running.worker_pid = Some(Pid::from_raw(value.parse().unwrap()));
        }
        if ready_line == Some(line.as_str()) {
            ready = true;
        }
    }
    if signal.is_some()
        && let Some(pid) = running.worker_pid
    {
        assert_eq!(
            kill(pid, None),
            Ok(()),
            "The worker must start before stopping."
        );
    }

    let devctl_pid = Pid::from_raw(i32::try_from(running.devctl.id()).unwrap());
    let stop_started = Instant::now();
    if let Some(signal) = signal {
        kill(devctl_pid, signal).expect("The signal must be sent.");
    }

    let deadline = stop_started + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = running.devctl.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "devctl did not stop in time.");
        thread::sleep(Duration::from_millis(10));
    };
    let elapsed = stop_started.elapsed();
    let expected_code = if signal.is_some() { 0 } else { 1 };
    assert_eq!(
        status.code(),
        Some(expected_code),
        "Unexpected devctl exit: {status}"
    );

    assert_eq!(
        kill(running.service_pid.unwrap(), None),
        Err(Errno::ESRCH),
        "The direct child must be gone and reaped."
    );
    if let Some(pid) = running.worker_pid {
        // Linux may retain an orphaned zombie until its new parent reaps it.
        // Check that the worker has terminated, rather than claiming to reap it.
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
                Ok(stat) => {
                    let state = stat.rsplit_once(") ").unwrap().1.as_bytes()[0];
                    if state == b'Z' || state == b'X' {
                        break;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
                Err(error) => panic!("Failed to check worker state: {error}"),
            }
            assert!(Instant::now() < deadline, "The worker is still running.");
            thread::sleep(Duration::from_millis(10));
        }
    }
    running.service_pid = None;
    running.worker_pid = None;

    // Bound pipe closure too: a surviving descendant can keep stdout open.
    let deadline = Instant::now() + Duration::from_secs(5);
    while !reader.is_finished() {
        assert!(Instant::now() < deadline, "Service output did not close.");
        thread::sleep(Duration::from_millis(10));
    }
    reader.join().unwrap();
    let lines: Vec<_> = receiver.try_iter().collect();
    if signal.is_some() {
        assert!(
            lines
                .iter()
                .any(|line| line == &format!("Stopping service '{service}'."))
        );
        assert!(
            lines
                .iter()
                .any(|line| line == &format!("Stopped service '{service}'."))
        );
    }
    assert!(
        !lines
            .iter()
            .any(|line| line.contains("exited unexpectedly"))
    );
    (lines, elapsed)
}

#[test]
fn sigint_stops_the_direct_child_and_succeeds() {
    assert_service_exit(Some(Signal::SIGINT), "long_running", false, None);
}

#[test]
fn sigterm_stops_the_direct_child_and_succeeds() {
    assert_service_exit(Some(Signal::SIGTERM), "long_running", false, None);
}

#[cfg(target_os = "linux")]
#[test]
fn stopping_terminates_the_service_and_its_worker() {
    assert_service_exit(Some(Signal::SIGTERM), "process_tree", true, None);
}

#[test]
fn term_runs_cleanup_before_stopping() {
    let (lines, elapsed) =
        assert_service_exit(Some(Signal::SIGTERM), "graceful", false, Some("Ready"));
    assert!(lines.iter().any(|line| line == "TERM handled"));
    assert!(!lines.iter().any(|line| line.contains("forcing stop")));
    assert!(
        elapsed < Duration::from_secs(3),
        "Graceful stop took {elapsed:?}"
    );
}

#[test]
fn ignored_term_is_force_killed_after_the_grace_period() {
    let (lines, elapsed) =
        assert_service_exit(Some(Signal::SIGTERM), "ignore_term", false, Some("Ready"));
    assert!(lines.iter().any(|line| line.contains("forcing stop")));
    assert!(
        elapsed >= Duration::from_millis(4900),
        "Grace period was only {elapsed:?}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn leader_exit_still_terminates_a_worker_that_ignores_term() {
    let (lines, elapsed) =
        assert_service_exit(Some(Signal::SIGTERM), "stubborn_worker", true, None);
    assert!(!lines.iter().any(|line| line.contains("forcing stop")));
    assert!(
        elapsed < Duration::from_secs(3),
        "Worker cleanup took {elapsed:?}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn natural_exit_terminates_workers_and_preserves_the_original_status() {
    for (service, expected) in [
        ("orphan_success", "exit status: 0"),
        ("orphan_failure", "exit status: 7"),
        ("orphan_signal", "signal: 15"),
    ] {
        let (lines, _) = assert_service_exit(None, service, true, None);
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with(&format!("Service '{service}' exited: {expected}"))),
            "Original status was lost for {service}: {lines:?}"
        );
    }
}
