#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
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
        let binary = format!("#!/bin/sh\necho 'devctl {candidate_version}'\n").into_bytes();
        fs::write(self.root.join("binary"), &binary).unwrap();
        let output = Command::new("sha256sum")
            .arg(self.root.join("binary"))
            .output()
            .unwrap();
        assert!(output.status.success());
        let digest = String::from_utf8(output.stdout)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .to_owned();
        let name = format!("devctl-v{version}-x86_64-unknown-linux-musl");
        let sums = format!("{digest}  {name}\n");
        fs::write(self.root.join("sums"), &sums).unwrap();
        let release = serde_json::json!({
            "tag_name": format!("v{version}"), "draft": false, "prerelease": false,
            "assets": [
                {"name": name, "state": "uploaded", "size": binary.len()},
                {"name": "SHA256SUMS", "state": "uploaded", "size": sums.len()}
            ]
        });
        fs::write(self.root.join("release"), release.to_string()).unwrap();
        fs::create_dir_all(self.root.join("mock")).unwrap();
        let curl = self.root.join("mock/curl");
        fs::write(
            &curl,
            r#"#!/usr/bin/env python3
import os, pathlib, sys
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
if url.startswith('https://api.github.com/repos/Jackie733/devctl/releases/'):
    name = 'release'
elif url.startswith('https://github.com/Jackie733/devctl/releases/download/'):
    name = 'sums' if url.endswith('/SHA256SUMS') else 'binary'
else:
    sys.exit(22)
if (root / ('fail-' + name)).exists():
    sys.stdout.buffer.write((root / name).read_bytes())
    sys.exit(22)
sys.stdout.buffer.write((root / name).read_bytes())
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
    assert_eq!(fixture.contents("devctl"), current);
    assert!(!fixture.home.join(".local/bin/devctl.previous").exists());
    let requests = fs::read_to_string(fixture.root.join("requests")).unwrap();
    assert_eq!(requests.lines().count(), 1);
    assert!(requests.ends_with("/releases/latest\n"));
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
    assert!(requests.contains("/releases/tags/v1.0.0\n"));
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
    for failure in ["metadata", "manifest", "binary", "checksum", "version"] {
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
            "metadata" => fs::write(fixture.root.join("fail-release"), "").unwrap(),
            "manifest" => fs::write(fixture.root.join("fail-sums"), "").unwrap(),
            "binary" => fs::write(fixture.root.join("fail-binary"), "").unwrap(),
            "checksum" => fs::write(
                fixture.root.join("binary"),
                "#!/bin/sh\ntouch \"$UPDATE_FIXTURE/executed-unverified\"\necho 'devctl 1.2.0'\n",
            )
            .unwrap(),
            _ => (),
        }
        let output = fixture.run(&["update"]);
        assert_eq!(output.status.code(), Some(1), "{failure}: {output:?}");
        assert_eq!(fixture.contents("devctl"), current, "{failure}");
        assert_eq!(fixture.contents("devctl.previous"), previous, "{failure}");
        assert!(!fixture.root.join("executed-unverified").exists());
        fixture.assert_clean();
    }
}

#[test]
fn api_rate_limits_report_retry_and_preserve_installation() {
    for message in [
        "API rate limit exceeded for a shared address.",
        "You have exceeded a secondary rate limit.",
    ] {
        let fixture = Fixture::new();
        let current = fixture.program("devctl", "1.1.0");
        let previous = fixture.program("devctl.previous", "1.0.0");
        fixture.release("1.2.0", "1.2.0");
        fs::write(
            fixture.root.join("release"),
            serde_json::json!({ "message": message }).to_string(),
        )
        .unwrap();
        fs::write(fixture.root.join("fail-release"), "").unwrap();
        let output = fixture.run(&["update", "--check"]);
        assert_eq!(output.status.code(), Some(1));
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("GitHub API rate limit exceeded"), "{error}");
        assert!(error.contains("Retry after the limit resets"));
        assert!(!error.contains("private repositories"));
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
}

#[test]
fn invalid_incomplete_or_oversized_metadata_is_rejected_before_asset_download() {
    for failure in [
        "json",
        "draft",
        "prerelease",
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
            "draft" | "prerelease" => value[failure] = true.into(),
            "missing" => value["assets"] = serde_json::json!([]),
            "duplicate" => {
                let duplicate = value["assets"][0].clone();
                value["assets"].as_array_mut().unwrap().push(duplicate);
            }
            "size" => value["assets"][0]["size"] = (129u64 * 1024 * 1024).into(),
            "tag" => value["tag_name"] = "v1.2.0-rc.1".into(),
            _ => (),
        }
        let metadata = match failure {
            "json" => "not json".into(),
            "oversized" => " ".repeat(1024 * 1024 + 1),
            _ => value.to_string(),
        };
        fs::write(path, metadata).unwrap();
        let output = fixture.run(&["update"]);
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
