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
    signal::unix::{SignalKind, signal},
    time::{sleep, timeout},
};

use crate::config::Service;

pub enum RunOutcome {
    Exited(ExitStatus),
    Stopped,
}

pub async fn run(id: &str, service: &Service) -> Result<RunOutcome, String> {
    let (program, args) = service
        .cmd
        .split_first()
        .ok_or_else(|| format!("Service '{id}' has no command."))?;

    let cwd = service
        .cwd
        .as_ref()
        .ok_or_else(|| format!("Service '{id}' has no resolved working directory."))?;

    let mut interrupt = signal(SignalKind::interrupt())
        .map_err(|error| format!("Failed to register SIGINT handler: {error}"))?;

    let mut terminate = signal(SignalKind::terminate())
        .map_err(|error| format!("Failed to register SIGTERM handler: {error}"))?;

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
        Some(()) = interrupt.recv() => {
            stop(id, &mut child, pgid).await
        }
        Some(()) = terminate.recv() => {
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
    // Keep the leader's PID until the final group signal has been sent.
    signal_group(id, pgid, Signal::SIGKILL)?;

    child
        .wait()
        .await
        .map_err(|error| format!("Failed to reap service '{id}': {error}"))
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
