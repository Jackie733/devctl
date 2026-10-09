use std::{path::Path, process::Command};

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_devctl"))
        .args(args)
        .current_dir(std::env::temp_dir())
        .output()
        .expect("The devctl binary must run.")
}

#[test]
fn help_and_no_arguments_succeed_without_loading_config() {
    for args in [&[][..], &["--help"][..], &["-h"][..]] {
        let output = run(args);
        assert!(output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty());
        let stdout = String::from_utf8(output.stdout).expect("Output must be UTF-8.");
        assert!(stdout.contains("Usage:"));
        assert!(stdout.contains("validate"));
        assert!(stdout.contains("plan"));
    }
}

#[test]
fn version_uses_package_metadata() {
    for flag in ["--version", "-V"] {
        let output = run(&[flag]);
        assert!(output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty());
        assert_eq!(
            output.stdout,
            format!("devctl {}\n", env!("CARGO_PKG_VERSION")).as_bytes()
        );
    }
}

#[test]
fn subcommand_help_succeeds_without_loading_config() {
    for command in ["validate", "plan"] {
        let output = run(&[command, "--help"]);
        assert!(output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty());
        let stdout = String::from_utf8(output.stdout).expect("Output must be UTF-8.");
        assert!(stdout.contains("Usage:"));
        assert!(stdout.contains("--config"));
    }
}

#[test]
fn invalid_arguments_exit_with_code_two() {
    for args in [
        &["--unknown"][..],
        &["--config"][..],
        &["validate", "extra"][..],
        &["unknown-command"][..],
    ] {
        let output = run(args);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).expect("Errors must be UTF-8.");
        assert!(stderr.contains("error:"));
    }
}

#[test]
fn config_option_works_before_and_after_subcommand() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/dependencies/chain.toml");
    let path = path.to_str().expect("The fixture path must be UTF-8.");
    let before = run(&["--config", path, "plan", "web"]);
    let after = run(&["plan", "web", "--config", path]);
    assert!(before.status.success(), "{before:?}");
    assert!(after.status.success(), "{after:?}");
    assert!(before.stderr.is_empty());
    assert!(after.stderr.is_empty());
    assert_eq!(before.stdout, after.stdout);
}

#[test]
fn business_errors_exit_with_code_one() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/dependencies/chain.toml");
    let path = path.to_str().expect("The fixture path must be UTF-8.");
    let output = run(&["--config", path, "plan", "missing"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("Errors must be UTF-8.");
    assert!(stderr.contains("Error: Unknown service 'missing'."));
}
