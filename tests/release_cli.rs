#[test]
fn maintainer_release_preparation_and_recovery() {
    let output = std::process::Command::new("python3")
        .arg("tests/support/release_command.py")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("run release-command tests");
    assert!(
        output.status.success(),
        "release-command tests failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
