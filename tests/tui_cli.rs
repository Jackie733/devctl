#![cfg(any(target_os = "linux", target_os = "macos"))]

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

#[test]
fn vim_navigation_help_and_resizing_keep_control_scopes_and_background_lifetime() {
    scenario("navigation");
}

#[test]
fn visual_line_copy_emits_exact_body_and_preserves_control_and_exit_scopes() {
    scenario("copying");
}

#[test]
fn mouse_text_copy_wheel_history_and_terminal_restoration_work_in_a_real_terminal() {
    scenario("mouse");
}

#[test]
fn wsl_copy_handles_unicode_failure_and_in_flight_navigation_without_touching_host_clipboard() {
    scenario("windows-copying");
}

#[test]
fn history_pages_pause_bookmarks_and_expiry_work_in_a_real_terminal() {
    scenario("history");
}

#[test]
fn disk_history_search_copy_and_fresh_entry_follow_work_in_a_real_terminal() {
    scenario("disk-history");
}

#[cfg(target_os = "macos")]
#[test]
fn mac_copy_handles_unicode_failure_and_in_flight_navigation_without_touching_host_clipboard() {
    scenario("mac-copying");
}

fn add_scenario(mode: &str) {
    let output = Command::new("python3")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/tui_add_pty.py"))
        .args([env!("CARGO_BIN_EXE_devctl"), mode])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{mode}: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn add_dialog_creates_first_workspace_runs_and_preserves_duplicate_projects() {
    add_scenario("manual");
    add_scenario("default");
}
#[test]
fn add_dialog_cancellation_and_clipped_confirmation_never_write_unreviewed_configuration() {
    add_scenario("cancel");
    add_scenario("small");
}
#[test]
fn add_dialog_selects_workspace_member_and_declared_script_without_version_management() {
    add_scenario("workspace");
}
#[test]
fn add_dialog_rechecks_source_and_target_after_review() {
    add_scenario("source_changed");
    add_scenario("target_changed");
}
#[test]
fn add_dialog_hot_adds_without_restarting_services_or_losing_logs() {
    add_scenario("running");
    add_scenario("concurrent");
}

#[test]
fn add_dialog_completes_directory_paths_and_cycles_candidates_in_a_real_terminal() {
    add_scenario("completion");
}

#[test]
fn tui_themes_symbols_and_reduced_motion_keep_controls_and_reading_usable() {
    scenario("appearance");
}
