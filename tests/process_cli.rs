#![cfg(unix)]

use std::{
    io::{BufRead, BufReader},
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
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
    service_pid: Option<String>,
}

impl Drop for RunningService {
    fn drop(&mut self) {
        if let Some(pid) = &self.service_pid {
            let _ = Command::new("kill")
                .args(["-s", "KILL", pid])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        let _ = self.devctl.kill();
        let _ = self.devctl.wait();
    }
}

fn assert_signal_stops_service(signal: &str) {
    let config = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/process.toml");
    let devctl = Command::new(env!("CARGO_BIN_EXE_devctl"))
        .arg("--config")
        .arg(config)
        .args(["run", "long_running"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("devctl must start.");
    let mut running = RunningService {
        devctl,
        service_pid: None,
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
    let started = receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("devctl must report startup within five seconds.");
    let pid = started
        .strip_prefix("Started service 'long_running' with PID ")
        .and_then(|value| value.strip_suffix('.'))
        .expect("Startup must include the service PID.");
    assert!(pid.parse::<u32>().is_ok());
    running.service_pid = Some(pid.to_owned());

    let delivered = Command::new("kill")
        .args(["-s", signal, &running.devctl.id().to_string()])
        .status()
        .expect("The signal must be sent.");
    assert!(delivered.success());

    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = running.devctl.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "devctl did not stop in time.");
        thread::sleep(Duration::from_millis(10));
    };
    assert!(
        status.success(),
        "User-requested stop must succeed: {status}"
    );

    let alive = Command::new("kill")
        .args(["-0", pid])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("The child PID must be checked.");
    assert!(
        !alive.success(),
        "The direct child must be gone and reaped."
    );
    running.service_pid = None;

    reader.join().unwrap();
    let lines: Vec<_> = receiver.try_iter().collect();
    assert!(
        lines
            .iter()
            .any(|line| line == "Stopping service 'long_running'.")
    );
    assert!(
        lines
            .iter()
            .any(|line| line == "Stopped service 'long_running'.")
    );
    assert!(
        !lines
            .iter()
            .any(|line| line.contains("exited unexpectedly"))
    );
}

#[test]
fn sigint_stops_the_direct_child_and_succeeds() {
    assert_signal_stops_service("INT");
}

#[test]
fn sigterm_stops_the_direct_child_and_succeeds() {
    assert_signal_stops_service("TERM");
}
