use std::path::Path;

use tokio::signal::unix::{SignalKind, signal};

use crate::{config, control_socket::ControlSocket, manager::Manager, run_lock::RunLock};

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
    let _socket = ControlSocket::bind(&path, &_run_lock)?;
    let mut manager = Manager::new(config.services);

    tokio::select! {
        _ = interrupt.recv() => {}
        _ = terminate.recv() => {}
    }

    manager.stop_all().await
}
