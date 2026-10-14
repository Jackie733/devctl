#![cfg(target_os = "linux")]
mod support;

use nix::{
    errno::Errno,
    sys::signal::{Signal, kill, killpg},
    unistd::{Pid, getsid},
};
use std::{
    fs,
    io::Read,
    path::Path,
    process::{Command, Output, Stdio},
    sync::{Arc, Barrier},
    thread,
    time::{Duration, Instant},
};
use support::ConfigFixture;

struct Workspace {
    config: ConfigFixture,
    pids: Vec<Pid>,
}
impl Workspace {
    fn new() -> Self {
        let config = ConfigFixture::new("process.toml");
        let mut value: toml::Value =
            toml::from_str(&fs::read_to_string(&config.path).unwrap()).unwrap();
        let template = value["services"]["long_running"].clone();
        let mut services = toml::Table::new();
        for id in ["web", "weapp", "middle"] {
            let mut service = template.clone();
            service["autostart"] = toml::Value::Boolean(true);
            service["cmd"] = toml::Value::Array(
                [
                    "sh",
                    "-c",
                    "printf '%s\\n' \"$$\" >> \"$1\"; exec sleep 60",
                    "sh",
                    config.path.with_file_name(id).to_str().unwrap(),
                ]
                .into_iter()
                .map(|v| toml::Value::String(v.into()))
                .collect(),
            );
            services.insert(id.into(), service);
        }
        value["services"] = toml::Value::Table(services);
        fs::write(&config.path, toml::to_string(&value).unwrap()).unwrap();
        Self {
            config,
            pids: Vec::new(),
        }
    }
    fn run(&self, args: &[&str]) -> Output {
        run(&self.config.path, args)
    }
    fn ok(&self, args: &[&str]) -> String {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "{:?}: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }
    fn edit(&self, edit: impl FnOnce(&mut toml::Value)) {
        let mut config = toml::from_str(&fs::read_to_string(&self.config.path).unwrap()).unwrap();
        edit(&mut config);
        fs::write(&self.config.path, toml::to_string(&config).unwrap()).unwrap();
    }
    fn pid(&mut self, id: &str) -> Pid {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(value) = fs::read_to_string(self.config.path.with_file_name(id))
                && let Some(value) = value.lines().last()
                && let Ok(value) = value.parse::<i32>()
            {
                let pid = Pid::from_raw(value);
                if kill(pid, None).is_err() {
                    assert!(Instant::now() < deadline, "Service {id} did not restart.");
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                if !self.pids.contains(&pid) {
                    self.pids.push(pid);
                }
                return pid;
            }
            assert!(Instant::now() < deadline, "Service {id} did not start.");
            thread::sleep(Duration::from_millis(10));
        }
    }
    fn stopped(&mut self, pid: Pid) {
        assert_eq!(kill(pid, None), Err(Errno::ESRCH), "Child not reaped.");
        self.pids.retain(|value| *value != pid);
    }
    fn socket(&self) -> std::path::PathBuf {
        self.config
            .path
            .with_file_name("devctl.toml.devctl/control.sock")
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = self.run(&["shutdown"]);
        for pid in &self.pids {
            let _ = killpg(*pid, Signal::SIGKILL);
        }
    }
}
fn run(path: &Path, args: &[&str]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_devctl"))
        .arg("--config")
        .arg(path)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("CLI command timed out: {args:?}");
        }
        thread::sleep(Duration::from_millis(10));
    };
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    child
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut stdout)
        .unwrap();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_end(&mut stderr)
        .unwrap();
    Output {
        status,
        stdout,
        stderr,
    }
}

#[test]
fn background_single_all_stop_restart_and_shutdown_keep_their_scope() {
    let mut workspace = Workspace::new();
    workspace.ok(&["start", "web"]);
    let web = workspace.pid("web");
    let status = workspace.ok(&["status"]);
    assert!(status.contains("web: Running"));
    assert!(status.contains("weapp: Stopped") && status.contains("middle: Stopped"));
    workspace.ok(&["start", "--all"]);
    let weapp = workspace.pid("weapp");
    let middle = workspace.pid("middle");
    assert_eq!(workspace.pid("web"), web);
    assert_eq!(
        fs::read_to_string(workspace.config.path.with_file_name("web"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    workspace.ok(&["stop", "web"]);
    workspace.stopped(web);
    assert_eq!(kill(weapp, None), Ok(()));
    assert_eq!(kill(middle, None), Ok(()));
    workspace.ok(&["start", "web"]);
    let web = workspace.pid("web");
    workspace.ok(&["stop", "--all"]);
    for pid in [web, weapp, middle] {
        workspace.stopped(pid);
    }
    assert!(
        workspace
            .ok(&["status"])
            .lines()
            .all(|line| line.ends_with("Stopped"))
    );
    workspace.ok(&["start", "middle"]);
    let middle = workspace.pid("middle");
    workspace.ok(&["shutdown"]);
    workspace.stopped(middle);
    assert!(!workspace.socket().exists());
    assert!(!workspace.run(&["status"]).status.success());
}

#[test]
fn cold_invalid_requests_and_read_only_commands_do_not_create_a_supervisor() {
    let workspace = Workspace::new();
    for args in [
        vec!["start", "web", "unknown"],
        vec!["stop", "--all"],
        vec!["status"],
        vec!["shutdown"],
    ] {
        assert!(!workspace.run(&args).status.success());
        assert!(!workspace.socket().exists());
        assert!(
            !workspace
                .config
                .path
                .with_file_name("devctl.toml.devctl.lock")
                .exists()
        );
        assert!(!workspace.config.path.with_file_name("web").exists());
    }
}

#[test]
fn concurrent_clients_create_one_manager_and_one_service_instance() {
    let mut workspace = Workspace::new();
    let barrier = Arc::new(Barrier::new(8));
    let mut clients = Vec::new();
    for _ in 0..8 {
        let path = workspace.config.path.clone();
        let barrier = barrier.clone();
        clients.push(thread::spawn(move || {
            barrier.wait();
            run(&path, &["start", "web"])
        }));
    }
    for client in clients {
        let output = client.join().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let web = workspace.pid("web");
    assert_eq!(
        fs::read_to_string(workspace.config.path.with_file_name("web"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    workspace.ok(&["shutdown"]);
    workspace.stopped(web);
}

#[test]
fn all_stop_remains_responsive_and_fences_new_starts_during_cleanup() {
    let mut workspace = Workspace::new();
    workspace.edit(|config| {
        config["services"]["web"]["cmd"][2] = toml::Value::String(
            "trap '' TERM; printf '%s\\n' \"$$\" >> \"$1\"; exec sleep 60".into(),
        );
    });
    workspace.ok(&["start", "web"]);
    let web = workspace.pid("web");
    let path = workspace.config.path.clone();
    let stopping = thread::spawn(move || run(&path, &["stop", "--all"]));
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let started = Instant::now();
        let status = workspace.ok(&["status"]);
        assert!(started.elapsed() < Duration::from_secs(1));
        if status.contains("web: Stopping") {
            break;
        }
        assert!(Instant::now() < deadline);
    }
    assert!(!workspace.run(&["start", "middle"]).status.success());
    assert!(!workspace.config.path.with_file_name("middle").exists());
    let output = stopping.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    workspace.stopped(web);
    workspace.ok(&["shutdown"]);
}

#[test]
fn failures_are_queryable_and_do_not_stop_survivors_or_reload_config() {
    let mut workspace = Workspace::new();
    workspace.edit(|config| {
        config["services"]["weapp"]["cmd"] = toml::Value::Array(vec![toml::Value::String(
            "devctl-missing-executable".into(),
        )]);
    });
    workspace.ok(&["start", "web", "weapp"]);
    let web = workspace.pid("web");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let status = workspace.ok(&["status"]);
        if status.contains("weapp: Failed") {
            assert!(status.contains("Failed to start service 'weapp'"));
            break;
        }
        assert!(Instant::now() < deadline);
    }
    fs::write(&workspace.config.path, "invalid config now").unwrap();
    assert!(workspace.ok(&["status"]).contains("web: Running"));
    assert_eq!(kill(web, None), Ok(()));
    workspace.ok(&["shutdown"]);
    workspace.stopped(web);
}

#[test]
fn supervisor_is_a_session_leader_with_null_stdio_and_sigterm_cleans_children() {
    let mut workspace = Workspace::new();
    workspace.ok(&["start", "web"]);
    let web = workspace.pid("web");
    let status = fs::read_to_string(format!("/proc/{web}/status")).unwrap();
    let parent = status
        .lines()
        .find_map(|line| line.strip_prefix("PPid:\t"))
        .unwrap()
        .parse::<i32>()
        .unwrap();
    let supervisor = Pid::from_raw(parent);
    assert_eq!(getsid(Some(supervisor)).unwrap(), supervisor);
    for fd in [0, 1, 2] {
        assert_eq!(
            fs::read_link(format!("/proc/{parent}/fd/{fd}")).unwrap(),
            Path::new("/dev/null")
        );
    }
    kill(supervisor, Signal::SIGTERM).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while workspace.socket().exists() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(10));
    }
    workspace.stopped(web);
}

#[test]
fn closing_the_launching_terminal_preserves_services_and_allows_reconnection() {
    let mut workspace = Workspace::new();
    let output = Command::new("python3")
        .arg("-c")
        .arg(
            r#"
import os, pty, signal, sys
signal.alarm(15)
pid, terminal = pty.fork()
if pid == 0:
    os.execv(sys.argv[1], [sys.argv[1], '--config', sys.argv[2], 'start', 'web'])
try:
    while os.read(terminal, 4096):
        pass
except OSError:
    pass
_, status = os.waitpid(pid, 0)
os.close(terminal)
assert os.waitstatus_to_exitcode(status) == 0, status
print(pid)
"#,
        )
        .arg(env!("CARGO_BIN_EXE_devctl"))
        .arg(&workspace.config.path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let launcher = String::from_utf8(output.stdout)
        .unwrap()
        .trim()
        .parse::<i32>()
        .unwrap();
    let web = workspace.pid("web");
    assert_ne!(getsid(Some(web)).unwrap(), Pid::from_raw(launcher));
    assert!(workspace.ok(&["status"]).contains("web: Running"));
    workspace.ok(&["shutdown"]);
    workspace.stopped(web);
}
