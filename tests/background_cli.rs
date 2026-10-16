#![cfg(any(target_os = "linux", target_os = "macos"))]
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
    let mut out_pipe = child.stdout.take().unwrap();
    let out_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        out_pipe.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let mut err_pipe = child.stderr.take().unwrap();
    let err_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        err_pipe.read_to_end(&mut bytes).unwrap();
        bytes
    });
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
    let stdout = out_reader.join().unwrap();
    let stderr = err_reader.join().unwrap();
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
        vec!["logs", "web"],
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

#[cfg(target_os = "linux")]
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

#[test]
fn recent_logs_survive_client_exit_label_sources_and_distinguish_service_instances() {
    let mut workspace = Workspace::new();
    workspace.edit(|config| {
        config["services"]["web"]["cmd"][2] = toml::Value::String(
            "printf '%s\\n' \"$$\" >> \"$1\"; printf 'hello-out\\n'; printf 'hello-err\\n' >&2; trap 'printf last-line\\n; exit 0' TERM; while :; do sleep 1; done".into());
    });
    workspace.ok(&["start", "web"]);
    let first = workspace.pid("web");
    let deadline = Instant::now() + Duration::from_secs(3);
    let before = loop {
        let logs = workspace.ok(&["logs", "web"]);
        if logs.contains("hello-out") && logs.contains("hello-err") {
            break logs;
        }
        assert!(Instant::now() < deadline);
    };
    assert!(before.contains("web#1 stdout") && before.contains("web#1 stderr"));
    let cursor = before
        .lines()
        .filter_map(|line| {
            line.split_once("seq=")
                .and_then(|(_, suffix)| suffix.split_once(']'))
                .and_then(|(value, _)| value.parse::<u64>().ok())
        })
        .max()
        .unwrap();
    workspace.ok(&["stop", "web"]);
    workspace.stopped(first);
    assert!(workspace.ok(&["logs", "web"]).contains("last-line"));
    let page = workspace.ok(&["logs", "web", "--after", &cursor.to_string()]);
    assert!(page.contains("last-line"));
    assert!(!page.contains("hello-out") && !page.contains("hello-err"));
    workspace.ok(&["start", "web"]);
    let second = workspace.pid("web");
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let logs = workspace.ok(&["logs", "web"]);
        if logs.contains("web#2 stdout") {
            assert!(logs.contains("web#1 stdout"));
            break;
        }
        assert!(Instant::now() < deadline);
    }
    assert!(!workspace.run(&["logs", "unknown"]).status.success());
    assert_eq!(
        workspace
            .run(&["logs", "web", "--limit", "501"])
            .status
            .code(),
        Some(2)
    );
    workspace.ok(&["shutdown"]);
    workspace.stopped(second);
}

#[test]
fn cli_reads_backward_pages_beyond_the_initial_tail_without_overlap() {
    let mut workspace = Workspace::new();
    let marker = workspace.config.path.with_file_name("web");
    workspace.edit(|config| {
        config["services"]["web"]["cmd"] = toml::Value::Array([
            "python3".to_owned(), "-u".into(), "-c".into(),
            "import os,sys,time\nfor i in range(1,251):\n print('history-'+str(i)); time.sleep(.001)\nopen(sys.argv[1],'w').write(str(os.getpid())+'\\n')\ntime.sleep(60)".into(),
            marker.to_str().unwrap().into(),
        ].into_iter().map(toml::Value::String).collect());
    });
    workspace.ok(&["start", "web"]);
    let web = workspace.pid("web");
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if workspace
            .ok(&["logs", "web", "--limit", "1"])
            .contains("history-250")
        {
            break;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(10));
    }
    for (args, start, end) in [
        (vec!["logs", "web"], 151, 250),
        (vec!["logs", "web", "--before", "151"], 51, 150),
        (vec!["logs", "web", "--before", "51"], 1, 50),
    ] {
        let output = workspace.run(&args);
        assert!(output.status.success(), "{output:?}");
        let text = String::from_utf8(output.stdout).unwrap();
        let actual: Vec<_> = text
            .lines()
            .map(|line| line.split_once("] ").unwrap().1)
            .collect();
        let expected: Vec<_> = (start..=end).map(|i| format!("history-{i}")).collect();
        assert_eq!(actual, expected);
        let notice = String::from_utf8(output.stderr).unwrap();
        if start == 1 {
            assert!(notice.contains("start of retained output"));
        } else {
            assert!(notice.contains(&format!("--before {start}")));
        }
    }
    assert!(workspace.ok(&["logs", "web", "--before", "1"]).is_empty());
    assert!(workspace.ok(&["logs", "web", "--after", "250"]).is_empty());
    workspace.ok(&["shutdown"]);
    workspace.stopped(web);
}

#[test]
fn excessive_output_is_bounded_sanitized_and_does_not_block_control() {
    let mut workspace = Workspace::new();
    let marker = workspace.config.path.with_file_name("web");
    workspace.edit(|config| {
        config["services"]["web"]["cmd"] = toml::Value::Array([
            "python3".to_owned(), "-c".into(),
            "import os,sys,time\nfor _ in range(15000): os.write(1,b'x'*256+b'\\n')\ntime.sleep(.3)\nos.write(1,b'\\x1b[31mred\\x1b[0m\\nbad-\\xff\\n'+b'L'*40000+b'\\n')\nos.write(2,b'from-stderr\\n')\nopen(sys.argv[1],'w').write(str(os.getpid())+'\\n')\ntime.sleep(60)".into(),
            marker.to_str().unwrap().into(),
        ].into_iter().map(toml::Value::String).collect());
    });
    workspace.ok(&["start", "web"]);
    let web = workspace.pid("web");
    let deadline = Instant::now() + Duration::from_secs(3);
    let output = loop {
        let output = workspace.run(&["logs", "web", "--limit", "500"]);
        assert!(output.status.success());
        if String::from_utf8_lossy(&output.stdout).contains("from-stderr") {
            break output;
        }
        assert!(Instant::now() < deadline);
    };
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!text.contains('\x1b'));
    assert!(text.contains('�') && text.contains("red") && text.contains("[truncated]"));
    assert!(text.len() < 100_000);
    assert!(String::from_utf8_lossy(&output.stderr).contains("dropped or evicted"));
    let before = Instant::now();
    assert!(workspace.ok(&["status"]).contains("web: Running"));
    assert!(before.elapsed() < Duration::from_secs(1));
    workspace.ok(&["stop", "--all"]);
    workspace.stopped(web);
    workspace.ok(&["shutdown"]);
}

#[test]
fn a_client_disconnect_does_not_cancel_an_accepted_stop() {
    use std::{io::Write, net::Shutdown, os::unix::net::UnixStream};
    let mut workspace = Workspace::new();
    workspace.edit(|config| {
        config["services"]["web"]["cmd"][2] = toml::Value::String(
            "trap '' TERM; printf '%s\\n' \"$$\" >> \"$1\"; exec sleep 60".into(),
        );
    });
    workspace.ok(&["start", "web"]);
    let web = workspace.pid("web");
    let mut client = UnixStream::connect(workspace.socket()).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    client.write_all(b"CONTROL1\n").unwrap();
    let mut header = [0; 9];
    client.read_exact(&mut header).unwrap();
    let mut length = [0; 4];
    client.read_exact(&mut length).unwrap();
    let mut identity = vec![0; u32::from_be_bytes(length) as usize];
    client.read_exact(&mut identity).unwrap();
    let body = b"action='stop'\ntargets=['web']\nall=false\n";
    client
        .write_all(&(body.len() as u32).to_be_bytes())
        .unwrap();
    client.write_all(body).unwrap();
    client.shutdown(Shutdown::Write).unwrap();
    drop(client);
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let status = workspace.ok(&["status"]);
        if status.contains("web: Stopped") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Disconnected client canceled cleanup."
        );
        thread::sleep(Duration::from_millis(20));
    }
    workspace.stopped(web);
    workspace.ok(&["shutdown"]);
}

#[test]
fn continuous_stdout_and_stderr_do_not_starve_status_or_stop() {
    let mut workspace = Workspace::new();
    let marker = workspace.config.path.with_file_name("web");
    workspace.edit(|config| {
        config["services"]["web"]["cmd"] = toml::Value::Array([
            "python3".to_owned(), "-c".into(),
            "import os,sys\nopen(sys.argv[1],'w').write(str(os.getpid())+'\\n')\nwhile True:\n os.write(1,(b'out '+b'x'*128+b'\\n')*128)\n os.write(2,(b'err '+b'y'*128+b'\\n')*128)".into(),
            marker.to_str().unwrap().into(),
        ].into_iter().map(toml::Value::String).collect());
    });
    workspace.ok(&["start", "web"]);
    let web = workspace.pid("web");
    for _ in 0..3 {
        let before = Instant::now();
        assert!(workspace.ok(&["status"]).contains("web: Running"));
        assert!(before.elapsed() < Duration::from_secs(2));
    }
    assert!(!workspace.ok(&["logs", "web", "--limit", "5"]).is_empty());
    let before = Instant::now();
    workspace.ok(&["stop", "--all"]);
    assert!(before.elapsed() < Duration::from_secs(3));
    workspace.stopped(web);
    workspace.ok(&["shutdown"]);
}

fn workspace_identity(path: &Path) -> String {
    use std::{io::Write, os::unix::net::UnixStream};
    let mut stream = UnixStream::connect(path).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    stream.write_all(b"CONTROL1\n").unwrap();
    let mut header = [0; 9];
    stream.read_exact(&mut header).unwrap();
    let mut length = [0; 4];
    stream.read_exact(&mut length).unwrap();
    let mut identity = vec![0; u32::from_be_bytes(length) as usize];
    stream.read_exact(&mut identity).unwrap();
    let request = b"action='status'\n";
    stream
        .write_all(&(request.len() as u32).to_be_bytes())
        .unwrap();
    stream.write_all(request).unwrap();
    stream.read_exact(&mut length).unwrap();
    let mut response = vec![0; u32::from_be_bytes(length) as usize];
    stream.read_exact(&mut response).unwrap();
    let response: toml::Value = toml::from_str(std::str::from_utf8(&response).unwrap()).unwrap();
    let identity = response["workspace_id"].as_str().unwrap();
    assert_eq!(identity.len(), 32);
    identity.into()
}

#[test]
fn workspace_instance_identity_survives_service_restarts_and_changes_on_manager_recreation() {
    let mut workspace = Workspace::new();
    workspace.ok(&["start", "web"]);
    let web = workspace.pid("web");
    let original = workspace_identity(&workspace.socket());
    workspace.ok(&["stop", "--all"]);
    workspace.stopped(web);
    assert_eq!(workspace_identity(&workspace.socket()), original);
    workspace.ok(&["shutdown"]);
    workspace.ok(&["start", "web"]);
    let web = workspace.pid("web");
    assert_ne!(workspace_identity(&workspace.socket()), original);
    workspace.ok(&["shutdown"]);
    workspace.stopped(web);
}

#[test]
fn startup_failure_is_reported_without_overwriting_an_unrelated_endpoint_file() {
    use std::os::unix::fs::DirBuilderExt;
    let workspace = Workspace::new();
    fs::DirBuilder::new()
        .mode(0o700)
        .create(workspace.socket().parent().unwrap())
        .unwrap();
    fs::write(workspace.socket(), "keep me").unwrap();
    let before = Instant::now();
    let output = workspace.run(&["start", "web"]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("non-socket"),
        "{output:?}"
    );
    assert!(before.elapsed() < Duration::from_secs(3));
    assert_eq!(fs::read_to_string(workspace.socket()).unwrap(), "keep me");
    assert!(!workspace.config.path.with_file_name("web").exists());
}
