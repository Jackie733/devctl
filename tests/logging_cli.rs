#![cfg(target_os = "linux")]
mod support;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    thread,
    time::{Duration, Instant},
};
use support::ConfigFixture;

struct Workspace {
    config: ConfigFixture,
    state: PathBuf,
}
impl Workspace {
    fn new(script: &str) -> Self {
        let config = ConfigFixture::new("process.toml");
        let state = config.path.with_file_name("state");
        let mut value: toml::Value =
            toml::from_str(&fs::read_to_string(&config.path).unwrap()).unwrap();
        value["logs"]["enabled"] = toml::Value::Boolean(true);
        value["services"] = toml::Value::Table(toml::Table::from_iter([(
            "web".into(),
            value["services"]["long_running"].clone(),
        )]));
        value["services"]["web"]["cmd"] = toml::Value::Array(
            ["python3", "-u", "-c", script]
                .into_iter()
                .map(|s| toml::Value::String(s.into()))
                .collect(),
        );
        fs::write(&config.path, toml::to_string(&value).unwrap()).unwrap();
        Self { config, state }
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_devctl"))
            .arg("--config")
            .arg(&self.config.path)
            .args(args)
            .env("XDG_STATE_HOME", &self.state)
            .output()
            .unwrap()
    }
    fn ok(&self, args: &[&str]) -> Output {
        let result = self.run(args);
        assert!(
            result.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        result
    }
    fn wait_log(&self, needle: &str) -> Output {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let result = self.ok(&["logs", "web", "--limit", "1"]);
            if String::from_utf8_lossy(&result.stdout).contains(needle) {
                return result;
            }
            assert!(Instant::now() < deadline, "Did not receive {needle}");
            thread::sleep(Duration::from_millis(40));
        }
    }
    fn session(&self) -> String {
        let result = self.ok(&["logs", "web", "--limit", "1"]);
        String::from_utf8_lossy(&result.stderr)
            .split("Session: ")
            .nth(1)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .to_owned()
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = self.run(&["shutdown"]);
    }
}

#[test]
fn logs_outside_memory_survive_shutdown_and_can_be_searched_and_exported_without_starting() {
    let w = Workspace::new(
        "import time\nfor i in range(4300):\n print('old needle 日志' if i == 0 else 'line-'+str(i))\n time.sleep(.001)\nprint('done')\ntime.sleep(60)",
    );
    w.ok(&["start", "web"]);
    w.wait_log("done");
    let saved = w.ok(&["logs", "web", "--after", "0", "--limit", "3"]);
    assert!(String::from_utf8_lossy(&saved.stdout).contains("old needle 日志"));
    assert!(!String::from_utf8_lossy(&saved.stderr).contains("degraded"));
    let session = w.session();
    w.ok(&["shutdown"]);
    let search = w.ok(&["logs", "web", "--contains", "old needle", "--after", "0"]);
    assert!(String::from_utf8_lossy(&search.stdout).contains("old needle 日志"));
    assert!(String::from_utf8_lossy(&search.stderr).contains(&session));
    // stdout is the selected query page, stderr carries scope, gaps and continuation.
    let export = w.config.path.with_file_name("excerpt.log");
    let redirected = Command::new(env!("CARGO_BIN_EXE_devctl"))
        .arg("--config")
        .arg(&w.config.path)
        .args([
            "logs",
            "web",
            "--session",
            &session,
            "--contains",
            "old needle",
            "--after",
            "0",
        ])
        .env("XDG_STATE_HOME", &w.state)
        .stdout(fs::File::create(&export).unwrap())
        .output()
        .unwrap();
    assert!(redirected.status.success());
    assert_eq!(fs::read(export).unwrap(), search.stdout);
    assert!(String::from_utf8_lossy(&redirected.stderr).contains(&session));
    let sessions = w.ok(&["log-sessions"]);
    let text = String::from_utf8_lossy(&sessions.stdout);
    assert!(text.contains(&session) && text.contains("closed") && text.contains("web"));
    assert!(
        !w.run(&["status"]).status.success(),
        "Historical lookup must not start a supervisor"
    );
    // The archived workspace remains readable after a project path disappears.
    let mut value: toml::Value =
        toml::from_str(&fs::read_to_string(&w.config.path).unwrap()).unwrap();
    for (_, project) in value["projects"].as_table_mut().unwrap().iter_mut() {
        project["path"] = toml::Value::String("/devctl-missing-project".into());
    }
    fs::write(&w.config.path, toml::to_string(&value).unwrap()).unwrap();
    assert!(
        String::from_utf8_lossy(
            &w.ok(&[
                "logs",
                "web",
                "--session",
                &session,
                "--after",
                "0",
                "--limit",
                "1"
            ])
            .stdout
        )
        .contains("old needle")
    );
}

#[test]
fn new_background_defaults_to_its_own_session_and_explicit_history_works_while_running() {
    let w =
        Workspace::new("import os,time\nprint('instance-pid='+str(os.getpid()))\ntime.sleep(60)");
    w.ok(&["start", "web"]);
    let first = w.wait_log("instance-pid=");
    let old = w.session();
    w.ok(&["shutdown"]);
    // Disabling new saves must not hide already retained historical sessions.
    let mut value: toml::Value =
        toml::from_str(&fs::read_to_string(&w.config.path).unwrap()).unwrap();
    value["logs"]["enabled"] = toml::Value::Boolean(false);
    fs::write(&w.config.path, toml::to_string(&value).unwrap()).unwrap();
    w.ok(&["start", "web"]);
    let second = w.wait_log("instance-pid=");
    let new = w.session();
    assert_ne!(old, new);
    assert_ne!(first.stdout, second.stdout);
    assert_eq!(
        w.ok(&["logs", "web", "--session", &old]).stdout,
        first.stdout
    );
    let socket = w
        .config
        .path
        .with_file_name("devctl.toml.devctl/control.sock");
    let hidden = socket.with_extension("hidden");
    fs::rename(&socket, &hidden).unwrap();
    let failed = w.run(&["logs", "web", "--session", &old]);
    fs::rename(hidden, socket).unwrap();
    assert!(
        !failed.status.success(),
        "A missing socket does not authorize offline reading while the run lock is held"
    );
    assert!(String::from_utf8_lossy(&failed.stderr).contains("Cannot connect"));
    assert!(
        !w.run(&["logs", "web", "--session", "../../invalid"])
            .status
            .success()
    );
}

#[test]
fn failed_storage_keeps_memory_logs_status_and_stop_available_and_reports_shutdown_loss() {
    let w = Workspace::new("import time\nprint('memory-still-works')\ntime.sleep(60)");
    fs::write(&w.state, "blocked directory").unwrap();
    w.ok(&["start", "web"]);
    let logs = w.wait_log("memory-still-works");
    assert!(String::from_utf8_lossy(&logs.stderr).contains("degraded"));
    assert!(String::from_utf8_lossy(&w.ok(&["status"]).stdout).contains("Running"));
    w.ok(&["stop", "web"]);
    assert!(String::from_utf8_lossy(&w.ok(&["status"]).stdout).contains("Stopped"));
    let shutdown = w.run(&["shutdown"]);
    assert!(!shutdown.status.success());
    assert!(String::from_utf8_lossy(&shutdown.stderr).contains("Log storage"));
    assert_eq!(fs::read_to_string(&w.state).unwrap(), "blocked directory");
}
