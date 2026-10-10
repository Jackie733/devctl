use std::process::{ExitStatus, Stdio};

use tokio::{
    process::{Child, Command},
    signal::unix::{SignalKind, signal},
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

    println!("Started service '{id}' with PID {pid}.");

    tokio::select! {
        Some(()) = interrupt.recv() => {
            stop(id, &mut child).await
        }
        Some(()) = terminate.recv() => {
            stop(id, &mut child).await
        }
        result = child.wait() => {
            let status = result.map_err(|error| format!("Failed to wait for service '{id}': {error}"))?;

            println!("Service '{id}' exited: {status}");

            Ok(RunOutcome::Exited(status))
        }
    }
}

async fn stop(id: &str, child: &mut Child) -> Result<RunOutcome, String> {
    println!("Stopping service '{id}'.");

    child
        .kill()
        .await
        .map_err(|error| format!("Failed to stop service '{id}': {error}"))?;

    println!("Stopped service '{id}'.");

    Ok(RunOutcome::Stopped)
}
