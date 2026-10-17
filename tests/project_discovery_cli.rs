use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

use serde_json::json;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    home: PathBuf,
    project: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "devctl-import-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let home = root.join("home");
        let project = root.join("project with spaces");
        fs::create_dir_all(project.join(".git")).unwrap();
        fs::create_dir(&home).unwrap();
        Self {
            root,
            home,
            project,
        }
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.project.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_devctl"))
            .args(args)
            .current_dir(&self.project)
            .env("HOME", &self.home)
            .output()
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn discovery_stays_read_only_but_invalid_existing_config_blocks_import_planning() {
    let fixture = Fixture::new();
    fixture.write(
        "package.json",
        &json!({
            "name":"demo", "packageManager":"npm@10.9.0", "scripts":{
                "dev":format!("printf executed > '{}'", fixture.root.join("script-executed").display()),
                "postinstall":format!("printf executed > '{}'", fixture.root.join("install-executed").display())
            }
        })
        .to_string(),
    );
    fixture.write(
        "devctl.toml",
        "intentionally invalid and must not be read by project discovery",
    );
    fixture.write(".env", "DO_NOT_READ_THIS_SECRET=private-value");
    let executable_dir = fixture.root.join("fake-tools");
    fs::create_dir(&executable_dir).unwrap();
    for name in ["node", "npm", "pnpm", "nvm", "git", "sh"] {
        let path = executable_dir.join(name);
        fs::write(
            &path,
            format!(
                "#!/bin/sh\nprintf executed > '{}'\nexit 97\n",
                fixture.root.join("tool-executed").display()
            ),
        )
        .unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let manifest = fs::read(fixture.project.join("package.json")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_devctl"))
        .args(["import", "."])
        .current_dir(&fixture.project)
        .env("HOME", &fixture.home)
        .env("PATH", &executable_dir)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("Existing configuration is invalid"));
    let text = String::from_utf8(output.stdout).unwrap();
    for expected in [
        "Read-only",
        "Project: .",
        "npm@10.9.0",
        "scripts.dev",
        "Scan: complete",
        "Configuration destination",
    ] {
        assert!(text.contains(expected), "{text}");
    }
    assert!(!text.contains("private-value"));
    assert!(!text.contains("satisfied"));
    assert!(!fixture.root.join("script-executed").exists());
    assert!(!fixture.root.join("install-executed").exists());
    assert!(!fixture.root.join("tool-executed").exists());
    assert!(fs::read_dir(&fixture.home).unwrap().next().is_none());
    assert_eq!(
        manifest,
        fs::read(fixture.project.join("package.json")).unwrap()
    );
    assert_eq!(
        fs::read_to_string(fixture.project.join("devctl.toml")).unwrap(),
        "intentionally invalid and must not be read by project discovery"
    );
}

#[test]
fn workspace_preview_separates_root_member_and_library_with_shared_install_location() {
    let fixture = Fixture::new();
    fixture.write(
        "package.json",
        r#"{"packageManager":"pnpm@10.3.0","scripts":{"dev":"turbo dev","dev:stop":"stop"}}"#,
    );
    fixture.write(
        "pnpm-workspace.yaml",
        "packages:\n  - 'apps/*'\n  - 'packages/*'\n",
    );
    fixture.write(
        "apps/web/package.json",
        r#"{"scripts":{"dev":"vite","start":"node server"}}"#,
    );
    fixture.write(
        "apps/api/package.json",
        r#"{"scripts":{"dev":"node server"}}"#,
    );
    fixture.write("packages/domain/package.json", r#"{"name":"domain"}"#);
    let output = fixture.run(&["import", "."]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("4 project(s)"));
    assert!(text.contains("Root scripts"));
    assert!(text.contains("Selection required"));
    assert!(text.contains("No declared scripts; not a runnable service"));
    assert!(text.contains("Script (explicit selection only): dev:stop"));
    assert_eq!(text.matches("Package manager: pnpm@10.3.0").count(), 4);
    let child = fixture.run(&["import", "apps/web"]);
    assert_eq!(child.status.code(), Some(1), "{child:?}");
    let text = String::from_utf8(child.stdout).unwrap();
    assert!(text.contains("1 project(s)"));
    assert!(text.contains("Package manager: pnpm@10.3.0"));
}

#[test]
fn declaration_conflicts_fail_with_evidence_without_hiding_other_projects() {
    let fixture = Fixture::new();
    fixture.write(
        "a/package.json",
        r#"{"packageManager":"pnpm@10.3.0","scripts":{"dev":"vite"}}"#,
    );
    fixture.write("a/package-lock.json", "{}");
    fixture.write("b/package.json", r#"{"scripts":{"dev":"node server"}}"#);
    let output = fixture.run(&["import", "."]);
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Conflicting package managers"));
    assert!(text.contains("Project: b"));
    assert!(text.contains("Package manager: unknown"));
    assert!(text.contains("incomplete or conflicting"));
}

#[test]
fn malformed_and_empty_directories_do_not_report_success() {
    let fixture = Fixture::new();
    let output = fixture.run(&["import", "."]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("No supported package.json")
    );
    fixture.write("package.json", "{not json");
    let output = fixture.run(&["import", "."]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("Invalid")
    );
    let output = fixture.run(&["import", "missing"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
}

#[test]
fn aliases_have_identical_results_and_child_symlinks_are_not_traversed() {
    let fixture = Fixture::new();
    fixture.write("package.json", "{}");
    symlink(&fixture.project, fixture.project.join("loop")).unwrap();
    let alias = fixture.root.join("alias");
    symlink(&fixture.project, &alias).unwrap();
    let direct = fixture.run(&["import", "."]);
    let linked = fixture.run(&["import", alias.to_str().unwrap()]);
    assert_eq!(direct.status.code(), Some(1));
    assert_eq!(direct.stdout, linked.stdout);
    assert_eq!(direct.stderr, linked.stderr);
}

#[test]
fn terminal_controls_in_untrusted_names_and_commands_are_not_emitted() {
    let fixture = Fixture::new();
    fixture.write(
        "package.json",
        &json!({"name":"demo\u{1b}[2J","scripts":{"dev":"echo \u{1b}[2J\rmalicious"}}).to_string(),
    );
    let output = fixture.run(&["import", "."]);
    assert_eq!(output.status.code(), Some(1));
    assert!(!output.stdout.contains(&0x1b));
    assert!(!output.stdout.contains(&b'\r'));
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("\\u{1b}")
    );
}

#[test]
fn apply_flags_are_exposed_and_explicit_config_selects_the_destination() {
    let fixture = Fixture::new();
    let help = fixture.run(&["import", "--help"]);
    assert!(help.status.success());
    let text = String::from_utf8(help.stdout).unwrap();
    assert!(text.contains("Preview project directories"));
    assert!(text.contains("--apply"));
    fixture.write(
        "package.json",
        r#"{"packageManager":"npm@11.0.0","scripts":{"dev":"node server"}}"#,
    );
    assert_eq!(fixture.run(&["import"]).status.code(), Some(2));
    assert_eq!(
        fixture.run(&["import", ".", "--apply"]).status.code(),
        Some(1)
    );
    let output = fixture.run(&["--config", "new.toml", "import", "."]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("Node selection required")
    );
    assert!(!fixture.project.join("new.toml").exists());
}
