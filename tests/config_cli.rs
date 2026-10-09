use std::{path::Path, process::Command};

fn validate_example(name: &str) -> std::process::Output {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/dependencies")
        .join(name);

    Command::new(env!("CARGO_BIN_EXE_devctl"))
        .args(["--config"])
        .arg(path)
        .arg("validate")
        .current_dir(std::env::temp_dir())
        .output()
        .expect("The devctl binary must run.")
}

#[test]
fn validates_chain_from_another_directory() {
    let output = validate_example("chain.toml");
    assert!(output.status.success(), "{output:?}");

    let stdout = String::from_utf8(output.stdout).expect("Output must be UTF-8.");
    assert!(stdout.contains("Configuration is valid."));
    assert!(stdout.contains("Services: 3"));
    assert!(stdout.contains("Depends on: db, condition: Started"));
    assert!(stdout.contains("Depends on: api, condition: Started"));
    assert!(output.stderr.is_empty());
}

#[test]
fn validates_shared_dependency() {
    let output = validate_example("shared.toml");
    assert!(output.status.success(), "{output:?}");

    let stdout = String::from_utf8(output.stdout).expect("Output must be UTF-8.");
    assert!(stdout.contains("Services: 4"));
    assert!(output.stderr.is_empty());
}

#[test]
fn rejects_cycle_through_cli() {
    let output = validate_example("cycle.toml");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("Errors must be UTF-8.");
    assert!(stderr.contains("Error: Dependency cycle: api -> db -> api"));
}

#[test]
fn rejects_cycle_in_separate_branch_through_cli() {
    let output = validate_example("separate-cycle.toml");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("Errors must be UTF-8.");
    assert!(stderr.contains("Error: Dependency cycle: queue -> worker -> queue"));
}
