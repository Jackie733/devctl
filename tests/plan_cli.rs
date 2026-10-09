use std::{path::Path, process::Command};

fn plan_example(name: &str, targets: &[&str]) -> std::process::Output {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(name);

    Command::new(env!("CARGO_BIN_EXE_devctl"))
        .arg("--config")
        .arg(path)
        .arg("plan")
        .args(targets)
        .current_dir(std::env::temp_dir())
        .output()
        .expect("The devctl binary must run.")
}

fn assert_plan(output: std::process::Output, expected: &[&str]) {
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("Output must be UTF-8.");
    assert_eq!(
        stdout.lines().next(),
        Some(format!("Startup plan: {}", expected.join(" -> ")).as_str())
    );
    let services: Vec<_> = stdout
        .lines()
        .filter_map(|line| line.strip_prefix("Service: "))
        .collect();
    assert_eq!(services, expected);
    assert!(stdout.contains("  Command: [\"echo\", \"db\"]"));
    assert!(stdout.contains("  Working directory: "));
}

#[test]
fn selects_targets_and_dependencies_from_another_directory() {
    let cases: &[(&[&str], &[&str])] = &[
        (&[], &["db", "api", "web"]),
        (&["web"], &["db", "api", "web"]),
        (&["api"], &["db", "api"]),
        (&["web", "api", "web"], &["db", "api", "web"]),
    ];
    for (targets, expected) in cases {
        assert_plan(plan_example("dependencies/chain.toml", targets), expected);
    }
}

#[test]
fn includes_shared_dependency_once() {
    assert_plan(
        plan_example("dependencies/shared.toml", &["web"]),
        &["db", "api", "worker", "web"],
    );
}

#[test]
fn unknown_target_does_not_print_a_partial_plan() {
    let output = plan_example("dependencies/chain.toml", &["web", "missing"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("Errors must be UTF-8.");
    assert!(stderr.contains("Error: Unknown service 'missing'."));
}

#[test]
fn cyclic_config_is_rejected_before_planning() {
    let output = plan_example("dependencies/cycle.toml", &["api"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("Errors must be UTF-8.");
    assert!(stderr.contains("Error: Dependency cycle: api -> db -> api"));
}

#[test]
fn empty_workspace_reports_no_selection() {
    let output = plan_example("minimal.toml", &[]);
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, b"No services selected.\n");
}
