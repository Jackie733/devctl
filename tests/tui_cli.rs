#![cfg(target_os = "linux")]

use std::{path::Path, process::Command};

fn scenario(name: &str) {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/tui_pty.py");
    let output = Command::new("python3")
        .arg(script)
        .arg(env!("CARGO_BIN_EXE_devctl"))
        .arg(name)
        .output()
        .expect("Python 3 must run the PTY acceptance checks.");
    assert!(
        output.status.success(),
        "{name}:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn tui_controls_logs_leaving_reattaching_and_shutdown_work_in_a_real_terminal() {
    scenario("lifecycle");
}

#[test]
fn tui_reconnects_without_recreating_or_replaying_and_signals_only_leave() {
    scenario("reconnect");
}

#[test]
fn tui_remains_responsive_during_stop_and_shows_isolated_failure() {
    scenario("slow");
}

#[test]
fn high_output_does_not_block_selection_or_leaving() {
    scenario("high");
}

#[test]
fn tui_discovers_both_home_configs_and_explicit_clients_attach_without_autostart() {
    scenario("discovery");
}
