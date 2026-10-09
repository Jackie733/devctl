#![cfg(unix)]

use std::{path::Path, process::Command};

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
