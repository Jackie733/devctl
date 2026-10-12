use std::{
    process::{ExitStatus, Stdio},
    time::Duration,
};

use nix::{
    errno::Errno,
    sys::{
        signal::{Signal, killpg},
        wait::{Id, WaitPidFlag, WaitStatus, waitid},
    },
    unistd::Pid,
};

use tokio::{
    process::{Child, Command},
    sync::oneshot,
    time::{sleep, timeout},
};

use crate::config::Service;

pub enum RunOutcome {
    Exited(ExitStatus),
    Stopped,
}

pub async fn run(
    id: &str,
    service: &Service,
    stop_request: oneshot::Receiver<()>,
) -> Result<RunOutcome, String> {
    let (program, args) = service
        .cmd
        .split_first()
        .ok_or_else(|| format!("Service '{id}' has no command."))?;

    let cwd = service
        .cwd
        .as_ref()
        .ok_or_else(|| format!("Service '{id}' has no resolved working directory."))?;

    let mut child = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .process_group(0)
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("Failed to start service '{id}': {error}"))?;

    let pid = child
        .id()
        .ok_or_else(|| format!("Service '{id}' has no PID after spawn."))?;

    let pgid = i32::try_from(pid)
        .map(Pid::from_raw)
        .map_err(|error| format!("Invalid process group for service '{id}': {error}"))?;

    println!("Started service '{id}' with PID {pid}.");

    tokio::select! {
        _ = stop_request => {
            stop(id, &mut child, pgid).await
        }
        result = wait_for_exit_without_reaping(id, pgid) => {
            result?;
            let status = kill_and_reap(id, &mut child, pgid).await?;

            println!("Service '{id}' exited: {status}");

            Ok(RunOutcome::Exited(status))
        }
    }
}

async fn stop(id: &str, child: &mut Child, pgid: Pid) -> Result<RunOutcome, String> {
    println!("Stopping service '{id}'.");

    signal_group(id, pgid, Signal::SIGTERM)?;

    let observed = match timeout(
        Duration::from_secs(5),
        wait_for_exit_without_reaping(id, pgid),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => {
            println!("Service '{id}' did not exit within 5 seconds; forcing stop.");
            Ok(())
        }
    };

    kill_and_reap(id, child, pgid).await?;
    observed?;

    println!("Stopped service '{id}'.");
    Ok(RunOutcome::Stopped)
}

async fn kill_and_reap(id: &str, child: &mut Child, pgid: Pid) -> Result<ExitStatus, String> {
    // Keep the leader's PID until the group has no live members.
    signal_group(id, pgid, Signal::SIGKILL)?;

    timeout(Duration::from_secs(1), async {
        wait_for_group_exit(id, pgid).await?;
        child
            .wait()
            .await
            .map_err(|error| format!("Failed to reap service '{id}': {error}"))
    })
    .await
    .map_err(|_| {
        format!("Process group for service '{id}' did not finish cleanup within 1 second.")
    })?
}

async fn wait_for_group_exit(id: &str, pgid: Pid) -> Result<(), String> {
    loop {
        let result = tokio::task::spawn_blocking(move || group_has_live_members(pgid))
            .await
            .map_err(|error| {
                format!("Failed to check process group for service '{id}': {error}")
            })?;

        let alive = result?;
        if !alive {
            return Ok(());
        }

        signal_group(id, pgid, Signal::SIGKILL)?;
        sleep(Duration::from_millis(20)).await;
    }
}

fn group_has_live_members(pgid: Pid) -> Result<bool, String> {
    let entries =
        std::fs::read_dir("/proc").map_err(|error| format!("Failed to read /proc: {error}"))?;

    for entry in entries {
        let entry = entry.map_err(|error| format!("Failed to read process entry: {error}"))?;

        if !entry
            .file_name()
            .to_string_lossy()
            .bytes()
            .all(|byte| byte.is_ascii_digit())
        {
            continue;
        }

        let path = entry.path().join("stat");
        let stat = match std::fs::read(&path) {
            Ok(stat) => stat,
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound
                    || error.raw_os_error() == Some(Errno::ESRCH as i32) =>
            {
                continue;
            }
            Err(error) => return Err(format!("Failed to read {}: {error}", path.display())),
        };

        if is_live_group_member(&stat, pgid)
            .map_err(|error| format!("{error} in {}", path.display()))?
        {
            return Ok(true);
        }
    }

    Ok(false)
}

fn is_live_group_member(stat: &[u8], pgid: Pid) -> Result<bool, &'static str> {
    let suffix = stat
        .iter()
        .rposition(|byte| *byte == b')')
        .and_then(|end| std::str::from_utf8(&stat[end + 1..]).ok())
        .ok_or("Invalid process status")?;

    let mut fields = suffix.split_whitespace();
    let state = fields.next().ok_or("Missing process state")?;
    let group = fields
        .nth(1)
        .and_then(|value| value.parse::<i32>().ok())
        .ok_or("Missing or invalid process group")?;

    Ok(group == pgid.as_raw() && !matches!(state, "Z" | "X" | "x"))
}

fn signal_group(id: &str, pgid: Pid, signal: Signal) -> Result<(), String> {
    match killpg(pgid, signal) {
        Ok(()) | Err(Errno::ESRCH) => Ok(()),
        Err(error) => Err(format!(
            "Failed to send {signal:?} to process group for service '{id}': {error}"
        )),
    }
}

async fn wait_for_exit_without_reaping(id: &str, pid: Pid) -> Result<(), String> {
    let flags = WaitPidFlag::WEXITED | WaitPidFlag::WNOHANG | WaitPidFlag::WNOWAIT;

    loop {
        match waitid(Id::Pid(pid), flags) {
            Ok(WaitStatus::StillAlive) | Err(Errno::EINTR) => {
                sleep(Duration::from_millis(20)).await;
            }
            Ok(_) => return Ok(()),
            Err(error) => {
                return Err(format!("Failed to observe service '{id}' exit: {error}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_names_can_contain_spaces_parentheses_and_non_utf8_bytes() {
        let stat = b"11 (worker (busy) \xff) S 7 42 0";
        assert_eq!(is_live_group_member(stat, Pid::from_raw(42)), Ok(true));
    }

    #[test]
    fn group_membership_uses_pgrp_instead_of_ppid() {
        let pgid = Pid::from_raw(42);
        assert_eq!(is_live_group_member(b"11 (cmd) S 42 7 0", pgid), Ok(false));
        assert_eq!(is_live_group_member(b"11 (cmd) S 7 42 0", pgid), Ok(true));
    }

    #[test]
    fn only_zombie_and_dead_members_are_excluded() {
        for state in ["R", "S", "D", "T", "t", "I", "Z", "X", "x"] {
            let stat = format!("11 (cmd) {state} 7 42 0");
            assert_eq!(
                is_live_group_member(stat.as_bytes(), Pid::from_raw(42)),
                Ok(!matches!(state, "Z" | "X" | "x")),
                "Unexpected liveness for state {state}"
            );
        }
    }

    #[test]
    fn invalid_status_is_an_error_instead_of_a_missing_member() {
        for stat in [
            b"11 cmd S 7 42".as_slice(),
            b"11 (cmd)",
            b"11 (cmd) S 7",
            b"11 (cmd) S 7 invalid",
            b"11 (cmd) S 7 \xff",
        ] {
            assert!(is_live_group_member(stat, Pid::from_raw(42)).is_err());
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn detects_the_current_live_process_group() {
        assert!(group_has_live_members(nix::unistd::getpgrp()).unwrap());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_missing_group_has_no_live_members() {
        assert!(!group_has_live_members(Pid::from_raw(i32::MAX)).unwrap());
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn stop_requests_and_sender_loss_clean_up_the_process_group() {
        use std::{fs, path::PathBuf};

        use nix::sys::signal::kill;

        struct Fixture {
            dir: PathBuf,
            pids: Option<(Pid, Pid)>,
        }

        impl Drop for Fixture {
            fn drop(&mut self) {
                if let Some((leader, worker)) = self.pids {
                    let _ = killpg(leader, Signal::SIGKILL);
                    let _ = kill(worker, Signal::SIGKILL);
                }
                let _ = fs::remove_dir_all(&self.dir);
            }
        }

        for send_request in [true, false] {
            let dir = std::env::temp_dir().join(format!(
                "devctl-stop-channel-{}-{send_request}",
                std::process::id()
            ));
            fs::create_dir(&dir).unwrap();
            let mut fixture = Fixture { dir, pids: None };
            let ready_path = fixture.dir.join("ready");
            let service = Service {
                project: "demo".into(),
                cmd: vec![
                    "sh".into(),
                    "-c".into(),
                    "sleep 60 & printf '%s %s\\n' \"$$\" \"$!\" > \"$1\"; wait".into(),
                    "sh".into(),
                    ready_path.to_str().unwrap().into(),
                ],
                cwd: Some(fixture.dir.clone()),
                autostart: false,
                depends_on: Vec::new(),
            };

            let (sender, receiver) = oneshot::channel();
            let running = run("controlled", &service, receiver);
            tokio::pin!(running);

            let ready = async {
                loop {
                    if let Ok(content) = fs::read_to_string(&ready_path) {
                        let pids: Vec<i32> = content
                            .split_whitespace()
                            .map(|pid| pid.parse().unwrap())
                            .collect();
                        if let [leader, worker] = pids.as_slice() {
                            return (Pid::from_raw(*leader), Pid::from_raw(*worker));
                        }
                    }
                    sleep(Duration::from_millis(10)).await;
                }
            };

            let pids = tokio::select! {
                result = &mut running => panic!("Service finished before the stop request: {}", result.is_ok()),
                result = timeout(Duration::from_secs(3), ready) => result.unwrap(),
            };
            fixture.pids = Some(pids);
            assert_eq!(kill(pids.0, None), Ok(()));
            assert_eq!(kill(pids.1, None), Ok(()));

            if send_request {
                sender.send(()).unwrap();
            } else {
                drop(sender);
            }

            let outcome = timeout(Duration::from_secs(8), running)
                .await
                .expect("Stop must finish within the cleanup budget.")
                .unwrap();
            assert!(matches!(outcome, RunOutcome::Stopped));
            assert_eq!(kill(pids.0, None), Err(Errno::ESRCH));
            assert!(!group_has_live_members(pids.0).unwrap());
            fixture.pids = None;
        }
    }
}
