#![cfg(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64")
))]

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

const TARGET: &str = if cfg!(target_os = "macos") {
    "aarch64-apple-darwin"
} else {
    "x86_64-unknown-linux-musl"
};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn newer_version() -> String {
    let current = semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
    format!("{}.0.0", current.major + 1)
}

struct Fixture {
    root: PathBuf,
    home: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "devctl-update-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let home = root.join("home");
        fs::create_dir_all(home.join(".local/bin")).unwrap();
        fs::create_dir(home.join(".config")).unwrap();
        fs::write(home.join(".config/keep"), "unchanged").unwrap();
        Self { root, home }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_devctl"));
        command
            .current_dir(&self.root)
            .env("HOME", &self.home)
            .env("UPDATE_FIXTURE", &self.root)
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.root.join("mock").display(),
                    std::env::var("PATH").unwrap()
                ),
            )
            .env_remove("BASH_ENV")
            .env_remove("ENV");
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }

    fn release(&self, version: &str, candidate_version: &str) -> Vec<u8> {
        let binary = format!("#!/bin/sh\nif [ -f \"$UPDATE_FIXTURE/slow-progress\" ]; then sleep 0.4; fi\necho 'devctl {candidate_version}'\n").into_bytes();
        fs::write(self.root.join("binary"), &binary).unwrap();
        let mut hasher = if cfg!(target_os = "macos") {
            let mut command = Command::new("shasum");
            command.args(["-a", "256"]);
            command
        } else {
            Command::new("sha256sum")
        };
        let output = hasher.arg(self.root.join("binary")).output().unwrap();
        assert!(output.status.success());
        let digest = String::from_utf8(output.stdout)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .to_owned();
        let name = format!("devctl-v{version}-{TARGET}");
        let release = serde_json::json!({
            "schema_version": 1, "version": version,
            "assets": [{"target": TARGET, "name": name,
                        "size": binary.len(), "sha256": digest}]
        });
        fs::write(self.root.join("release"), release.to_string()).unwrap();
        fs::create_dir_all(self.root.join("mock")).unwrap();
        let curl = self.root.join("mock/curl");
        fs::write(
            &curl,
            r#"#!/usr/bin/env python3
import os, pathlib, sys, time
root = pathlib.Path(os.environ['UPDATE_FIXTURE'])
args = sys.argv[1:]
assert args[0] == '--disable'
assert args[args.index('--proto') + 1] == '=https'
assert args[args.index('--proto-redir') + 1] == '=https'
assert '--fail-with-body' in args and '--location' in args
assert '--connect-timeout' in args and '--max-time' in args
url = args[-1]
with (root / 'requests').open('a') as log:
    log.write(url + '\n')
base = 'https://github.com/Jackie733/devctl/releases/'
if url == base + 'latest/download/update.json' or (url.startswith(base + 'download/v') and url.endswith('/update.json')):
    name = 'release'
elif url.startswith(base + 'download/v') and '/devctl-v' in url:
    name = 'binary'
else:
    sys.exit(22)
data = (root / name).read_bytes()
slow = (root / 'slow-progress').exists()
if slow:
    time.sleep(0.3)
if (root / ('fail-' + name)).exists():
    sys.stderr.write('fixture diagnostic begins')
    sys.stderr.flush()
    if slow:
        time.sleep(0.2)
    sys.stderr.write(' and ends' + ('' if slow else '\n'))
    sys.stdout.buffer.write(data)
    sys.exit(22)
if slow and name == 'binary':
    step = max(1, len(data) // 4)
    for offset in range(0, len(data), step):
        sys.stdout.buffer.write(data[offset:offset + step])
        sys.stdout.buffer.flush()
        time.sleep(0.15)
else:
    sys.stdout.buffer.write(data)
"#,
        )
        .unwrap();
        fs::set_permissions(curl, fs::Permissions::from_mode(0o755)).unwrap();
        binary
    }

    fn assert_clean(&self) {
        assert_eq!(
            fs::read_to_string(self.home.join(".config/keep")).unwrap(),
            "unchanged"
        );
        for entry in fs::read_dir(self.home.join(".local/bin")).unwrap() {
            let name = entry.unwrap().file_name().to_string_lossy().into_owned();
            assert!(!name.starts_with(".devctl-download-"), "{name}");
            assert!(
                !name.starts_with(".devctl-update.") || name == ".devctl-update.lock",
                "{name}"
            );
        }
    }

    fn program(&self, name: &str, version: &str) -> Vec<u8> {
        let body = format!("#!/bin/sh\necho 'devctl {version}'\n").into_bytes();
        let path = self.home.join(".local/bin").join(name);
        fs::write(&path, &body).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        body
    }

    fn contents(&self, name: &str) -> Vec<u8> {
        fs::read(self.home.join(".local/bin").join(name)).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn rollback_is_offline_and_independent_of_workspace_config() {
    let fixture = Fixture::new();
    let current = fixture.program("devctl", "1.2.0");
    let previous = fixture.program("devctl.previous", "1.1.0");
    // An explicit missing config must not be resolved, even during rollback.
    let output = fixture.run(&[
        "--config",
        "/missing/workspace.toml",
        "update",
        "--rollback",
    ]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(fixture.contents("devctl"), previous);
    assert_eq!(fixture.contents("devctl.previous"), current);
    assert_eq!(
        fs::read_to_string(fixture.home.join(".config/keep")).unwrap(),
        "unchanged"
    );
    fixture.assert_clean();
}

#[test]
fn check_only_reads_metadata_and_never_changes_installation() {
    let fixture = Fixture::new();
    let current = fixture.program("devctl", "1.0.0");
    let version = newer_version();
    fixture.release(&version, &version);
    let output = fixture.run(&["update", "--check"]);
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("An update is available"));
    let diagnostics = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostics.contains("Checking release...") && diagnostics.contains("Release checked"));
    assert!(!diagnostics.contains('\x1b') && !diagnostics.contains('\r'));
    assert!(!diagnostics.contains("Downloading") && !diagnostics.contains("Verifying"));
    assert_eq!(fixture.contents("devctl"), current);
    assert!(!fixture.home.join(".local/bin/devctl.previous").exists());
    let requests = fs::read_to_string(fixture.root.join("requests")).unwrap();
    assert_eq!(requests.lines().count(), 1);
    assert_eq!(
        requests,
        "https://github.com/Jackie733/devctl/releases/latest/download/update.json\n"
    );
    fixture.assert_clean();
}

#[test]
fn latest_install_explicit_downgrade_and_rollback_preserve_config() {
    let fixture = Fixture::new();
    let initial = fixture.program("devctl", "1.1.0");
    let version = newer_version();
    let next = fixture.release(&version, &version);
    let output = fixture.run(&["--config", "/missing.toml", "update"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(fixture.contents("devctl"), next);
    assert_eq!(fixture.contents("devctl.previous"), initial);
    let older = fixture.release("1.0.0", "1.0.0");
    let output = fixture.run(&["update", "--version", "v1.0.0"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(fixture.contents("devctl"), older);
    assert_eq!(fixture.contents("devctl.previous"), next);
    let requests = fs::read_to_string(fixture.root.join("requests")).unwrap();
    assert_eq!(
        requests.lines().collect::<Vec<_>>(),
        vec![
            "https://github.com/Jackie733/devctl/releases/latest/download/update.json".to_owned(),
            format!(
                "https://github.com/Jackie733/devctl/releases/download/v{version}/devctl-v{version}-{TARGET}"
            ),
            "https://github.com/Jackie733/devctl/releases/download/v1.0.0/update.json".to_owned(),
            format!(
                "https://github.com/Jackie733/devctl/releases/download/v1.0.0/devctl-v1.0.0-{TARGET}"
            ),
        ]
    );
    let output = fixture.run(&["update", "--rollback"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(fixture.contents("devctl"), next);
    fixture.assert_clean();
}

#[test]
fn latest_does_not_implicitly_downgrade_or_discard_backup() {
    let fixture = Fixture::new();
    let current = fixture.program("devctl", env!("CARGO_PKG_VERSION"));
    let previous = fixture.program("devctl.previous", "0.1.0");
    fixture.release("1.0.0", "1.0.0");
    let output = fixture.run(&["update"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(fixture.contents("devctl"), current);
    assert_eq!(fixture.contents("devctl.previous"), previous);
    assert_eq!(
        fs::read_to_string(fixture.root.join("requests"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    fixture.assert_clean();
}

#[test]
fn failed_download_checksum_and_candidate_checks_preserve_program_and_backup() {
    for failure in ["manifest", "binary", "checksum", "version", "short", "long"] {
        let fixture = Fixture::new();
        let current = fixture.program("devctl", "1.1.0");
        let previous = fixture.program("devctl.previous", "1.0.0");
        let version = newer_version();
        fixture.release(
            &version,
            if failure == "version" {
                "0.0.0"
            } else {
                &version
            },
        );
        match failure {
            "manifest" => fs::write(fixture.root.join("fail-release"), "").unwrap(),
            "binary" => fs::write(fixture.root.join("fail-binary"), "").unwrap(),
            "checksum" => {
                let path = fixture.root.join("release");
                let mut value: serde_json::Value =
                    serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                value["assets"][0]["sha256"] = "0".repeat(64).into();
                fs::write(path, value.to_string()).unwrap();
            }
            "short" => fs::write(fixture.root.join("binary"), "").unwrap(),
            "long" => fs::write(fixture.root.join("binary"), vec![b'x'; 1000]).unwrap(),
            _ => (),
        }
        let output = fixture.run(&["update"]);
        assert_eq!(output.status.code(), Some(1), "{failure}: {output:?}");
        assert_eq!(fixture.contents("devctl"), current, "{failure}");
        assert_eq!(fixture.contents("devctl.previous"), previous, "{failure}");
        if failure == "checksum" {
            assert!(String::from_utf8_lossy(&output.stderr).contains("SHA-256 mismatch"));
        }
        fixture.assert_clean();
    }
}

#[test]
fn missing_manifest_reports_external_installer_without_api_fallback() {
    let fixture = Fixture::new();
    let current = fixture.program("devctl", "1.1.0");
    let previous = fixture.program("devctl.previous", "1.0.0");
    fixture.release("1.2.0", "1.2.0");
    fs::write(fixture.root.join("release"), "<html>Not Found</html>").unwrap();
    fs::write(fixture.root.join("fail-release"), "").unwrap();
    let output = fixture.run(&["update", "--check", "--version", "1.2.0"]);
    assert_eq!(output.status.code(), Some(1));
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("Cannot fetch update.json"), "{error}");
    assert!(error.contains("external installer"));
    assert!(error.contains("program and backup are unchanged"));
    assert!(!error.contains("<html>"));
    assert_eq!(fixture.contents("devctl"), current);
    assert_eq!(fixture.contents("devctl.previous"), previous);
    assert_eq!(
        fs::read_to_string(fixture.root.join("requests")).unwrap(),
        "https://github.com/Jackie733/devctl/releases/download/v1.2.0/update.json\n"
    );
    fixture.assert_clean();
}

#[test]
fn invalid_incomplete_or_oversized_metadata_is_rejected_before_asset_download() {
    for failure in [
        "json",
        "schema",
        "prerelease",
        "prefix",
        "field",
        "target",
        "path",
        "digest",
        "nonhex",
        "zero",
        "missing",
        "duplicate",
        "size",
        "oversized",
        "tag",
    ] {
        let fixture = Fixture::new();
        let current = fixture.program("devctl", "1.1.0");
        fixture.release("1.2.0", "1.2.0");
        let path = fixture.root.join("release");
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        match failure {
            "schema" => value["schema_version"] = 2.into(),
            "prerelease" => value["version"] = "1.2.0-rc.1".into(),
            "prefix" => value["version"] = "v1.2.0".into(),
            "field" => {
                value.as_object_mut().unwrap().remove("version");
            }
            "target" => value["assets"][0]["target"] = "aarch64-unknown-linux-musl".into(),
            "path" => value["assets"][0]["name"] = "../devctl".into(),
            "digest" => value["assets"][0]["sha256"] = "bad".into(),
            "nonhex" => value["assets"][0]["sha256"] = "z".repeat(64).into(),
            "zero" => value["assets"][0]["size"] = 0.into(),
            "missing" => value["assets"] = serde_json::json!([]),
            "duplicate" => {
                let duplicate = value["assets"][0].clone();
                value["assets"].as_array_mut().unwrap().push(duplicate);
            }
            "size" => value["assets"][0]["size"] = (129u64 * 1024 * 1024).into(),
            "tag" => value["version"] = "1.3.0".into(),
            _ => (),
        }
        let metadata = match failure {
            "json" => "not json".into(),
            "oversized" => " ".repeat(64 * 1024 + 1),
            _ => value.to_string(),
        };
        fs::write(path, metadata).unwrap();
        let output = fixture.run(&["update", "--version", "1.2.0"]);
        assert_eq!(output.status.code(), Some(1), "{failure}: {output:?}");
        assert_eq!(fixture.contents("devctl"), current);
        assert_eq!(
            fs::read_to_string(fixture.root.join("requests"))
                .unwrap()
                .lines()
                .count(),
            1
        );
        fixture.assert_clean();
    }
}

#[test]
fn rollback_failure_preserves_installation() {
    let fixture = Fixture::new();
    let current = fixture.program("devctl", "1.2.0");
    let output = fixture.run(&["update", "--rollback"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("No previous version"));
    assert_eq!(fixture.contents("devctl"), current);
}

#[test]
fn invalid_home_is_rejected_before_install_or_rollback() {
    let fixture = Fixture::new();
    for args in [&["update", "--check"][..], &["update", "--rollback"][..]] {
        let output = fixture
            .command()
            .args(args)
            .env("HOME", "relative")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains("HOME must be an absolute path"));
        assert!(!fixture.root.join("relative").exists());
    }
}

#[test]
fn update_arguments_and_version_errors_do_not_load_workspace_config() {
    let fixture = Fixture::new();
    for args in [
        &["update", "--rollback", "--check"][..],
        &["update", "--rollback", "--version", "1.0.0"][..],
    ] {
        assert_eq!(fixture.run(args).status.code(), Some(2));
    }
    for version in ["../bad", "1.0.0-rc.1", "1.0.0+local"] {
        let output = fixture.run(&["update", "--version", version]);
        assert_eq!(output.status.code(), Some(1));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("workspace configuration"));
    }
    let output = fixture.run(&["update", "--help"]);
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(help.contains("--check") && help.contains("--version") && help.contains("--rollback"));
}

#[test]
fn update_progress_works_in_real_terminals_and_falls_back_without_control_sequences() {
    for mode in [
        "interactive",
        "mono",
        "dumb",
        "redirect",
        "stderr-redirect",
        "narrow",
        "failure",
        "check",
        "current",
        "rollback",
    ] {
        let fixture = Fixture::new();
        let current = fixture.program("devctl", "1.0.0");
        let previous = fixture.program("devctl.previous", "0.9.0");
        let version = if mode == "current" {
            env!("CARGO_PKG_VERSION").to_owned()
        } else {
            newer_version()
        };
        fixture.release(&version, &version);
        fs::write(fixture.root.join("slow-progress"), "").unwrap();
        if mode == "failure" {
            fs::write(fixture.root.join("fail-binary"), "").unwrap();
        }
        let output = Command::new("python3")
            .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/support/update_pty.py"))
            .arg(env!("CARGO_BIN_EXE_devctl"))
            .arg(&fixture.root)
            .arg(mode)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{mode}:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if matches!(mode, "failure" | "check" | "current") {
            assert_eq!(fixture.contents("devctl"), current);
            assert_eq!(fixture.contents("devctl.previous"), previous);
        }
        fixture.assert_clean();
    }
}
