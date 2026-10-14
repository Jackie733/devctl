use std::path::Path;

use tokio::{
    signal::unix::{SignalKind, signal},
    task::JoinSet,
};

use crate::{config, control_socket::ControlSocket, manager::Manager, probe, run_lock::RunLock};

const MAX_CLIENTS: usize = 16;

pub async fn run(path: &Path) -> Result<(), String> {
    let path = path
        .canonicalize()
        .map_err(|error| format!("Failed to resolve config path {}: {error}", path.display()))?;
    let config = config::load(&path)?;

    let mut interrupt = signal(SignalKind::interrupt())
        .map_err(|error| format!("Failed to register SIGINT handler: {error}"))?;
    let mut terminate = signal(SignalKind::terminate())
        .map_err(|error| format!("Failed to register SIGTERM handler: {error}"))?;

    let _run_lock = RunLock::acquire(&path)?;
    let socket = ControlSocket::bind(&path, &_run_lock)?;
    let mut manager = Manager::new(config.services);
    let mut clients = JoinSet::new();

    let outcome = loop {
        tokio::select! {
            _ = interrupt.recv() => break Ok(()),
            _ = terminate.recv() => break Ok(()),
            accepted = socket.accept(), if clients.len() < MAX_CLIENTS => {
                match accepted {
                    Ok(stream) => {
                        let config_path = path.clone();
                        clients.spawn(async move { probe::respond(stream, &config_path).await });
                    }
                    Err(error) => break Err(format!("Failed to accept control connection: {error}")),
                }
            }
            Some(result) = clients.join_next(), if !clients.is_empty() => {
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => eprintln!("Probe failed: {error}"),
                    Err(error) => eprintln!("Probe task failed: {error}"),
                }
            }
        }
    };

    clients.shutdown().await;
    match (outcome, manager.stop_all().await) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(error), Err(cleanup)) => Err(format!("{error}\n{cleanup}")),
    }
}
