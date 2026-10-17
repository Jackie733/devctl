use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    home: PathBuf,
    cwd: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "devctl-discovery-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let home = root.join("home");
        let cwd = root.join("unrelated-directory");
        fs::create_dir_all(home.join(".config/devctl")).unwrap();
        fs::create_dir(&cwd).unwrap();
        Self { root, home, cwd }
    }

    fn write(&self, path: &Path, name: &str) {
        fs::write(path, format!("version = 1\n[workspace]\nname = '{name}'\n")).unwrap();
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_devctl"));
        command
            .args(args)
            .current_dir(&self.cwd)
            .env("HOME", &self.home);
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }

    fn workspace(&self, args: &[&str], expected: &str) {
        let output = self.run(args);
        assert!(output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty());
        assert!(
            String::from_utf8(output.stdout)
                .unwrap()
                .contains(&format!("Workspace: {expected}\n"))
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn discovers_both_home_locations_from_an_unrelated_directory() {
    let fixture = Fixture::new();
    let home = fixture.home.join("devctl.toml");
    fixture.write(&home, "home");
    fixture.workspace(&["validate"], "home");
    let config = fixture.home.join(".config/devctl/devctl.toml");
    fs::rename(home, config).unwrap();
    fixture.workspace(&["validate"], "home");
    assert!(!fixture.cwd.join("devctl.toml").exists());
}

#[test]
fn explicit_path_then_cwd_then_home_determine_the_workspace_without_merging() {
    let fixture = Fixture::new();
    let local = fixture.cwd.join("devctl.toml");
    let home = fixture.home.join("devctl.toml");
    let config = fixture.home.join(".config/devctl/devctl.toml");
    let explicit = fixture.root.join("explicit.toml");
    for (path, name) in [
        (&local, "local"),
        (&home, "home"),
        (&config, "config"),
        (&explicit, "explicit"),
    ] {
        fixture.write(path, name);
    }
    fixture.workspace(&["validate"], "local");
    fixture.workspace(
        &["--config", explicit.to_str().unwrap(), "validate"],
        "explicit",
    );
    fs::remove_file(local).unwrap();
    fixture.workspace(&["validate"], "home");
    fs::remove_file(home).unwrap();
    fixture.workspace(&["validate"], "config");
}

#[test]
fn missing_config_lists_search_locations_and_does_not_start_a_workspace() {
    let fixture = Fixture::new();
    for args in [&["validate"][..], &["status"], &["start", "--all"]] {
        let output = fixture.run(args);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.contains("No workspace configuration found"));
        assert!(error.contains("--config PATH"));
        for path in [
            fixture.cwd.join("devctl.toml"),
            fixture.home.join("devctl.toml"),
            fixture.home.join(".config/devctl/devctl.toml"),
        ] {
            assert!(error.contains(path.to_str().unwrap()), "{error}");
            assert!(!path.exists());
        }
    }
    for args in [&[][..], &["tui"][..]] {
        let output = fixture.run(args);
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains("TUI requires a terminal"));
    }
    assert_eq!(fs::read_dir(&fixture.cwd).unwrap().count(), 0);
    assert_eq!(
        fs::read_dir(fixture.home.join(".config/devctl"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn an_invalid_preferred_file_is_not_replaced_with_another_workspace() {
    let fixture = Fixture::new();
    let preferred = fixture.home.join("devctl.toml");
    let fallback = fixture.home.join(".config/devctl/devctl.toml");
    fixture.write(&fallback, "fallback");
    fs::write(&preferred, "not valid toml").unwrap();
    let output = fixture.run(&["validate"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("Failed to parse")
    );
    fs::remove_file(&preferred).unwrap();
    fs::create_dir(&preferred).unwrap();
    let output = fixture.run(&["validate"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("not a regular file")
    );
}

#[cfg(unix)]
#[test]
fn a_broken_config_symlink_is_reported_instead_of_selecting_the_fallback() {
    let fixture = Fixture::new();
    fixture.write(&fixture.home.join(".config/devctl/devctl.toml"), "fallback");
    std::os::unix::fs::symlink(
        fixture.root.join("missing"),
        fixture.home.join("devctl.toml"),
    )
    .unwrap();
    let output = fixture.run(&["validate"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("Failed to inspect config")
    );
}

#[test]
fn explicit_missing_path_never_falls_back_to_a_valid_home_config() {
    let fixture = Fixture::new();
    fixture.write(&fixture.home.join("devctl.toml"), "home");
    let missing = fixture.root.join("missing.toml");
    let output = fixture.run(&["validate", "--config", missing.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains(missing.to_str().unwrap())
    );
}

#[test]
fn help_and_version_do_not_require_a_config() {
    let fixture = Fixture::new();
    for args in [
        &["--help"][..],
        &["--version"],
        &["validate", "--help"],
        &["tui", "--help"],
    ] {
        let output = fixture.run(args);
        assert!(output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn without_home_a_local_config_still_works_and_a_missing_one_is_explained() {
    let fixture = Fixture::new();
    let local = fixture.cwd.join("devctl.toml");
    fixture.write(&local, "local");
    let output = fixture
        .command(&["validate"])
        .env_remove("HOME")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    fs::remove_file(local).unwrap();
    let output = fixture
        .command(&["validate"])
        .env_remove("HOME")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("HOME is not set")
    );
}
