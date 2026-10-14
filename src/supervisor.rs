use std::{path::Path, time::Duration};

use tokio::{
    signal::unix::{SignalKind, signal},
    sync::{mpsc, oneshot},
    task::JoinSet,
    time::timeout,
};

use crate::{
    config,
    control::{self, Command, Request, Response},
    control_socket::ControlSocket,
    manager::{Manager, ServiceState},
    run_lock::RunLock,
    startup,
};

const MAX_CLIENTS: usize = 16;

struct PendingStop {
    ids: Vec<String>,
    errors: Vec<String>,
    reply: oneshot::Sender<Response>,
}

fn snapshot(manager: &mut Manager, error: Option<String>) -> Response {
    Response {
        services: manager.statuses().clone(),
        error,
    }
}

fn validate_selection(targets: &[String], all: bool) -> Result<(), String> {
    if all != targets.is_empty() {
        return Err("Select explicit service IDs or --all.".into());
    }
    Ok(())
}

fn dispatch(
    command: Command,
    manager: &mut Manager,
    pending: &mut Vec<PendingStop>,
    shutting_down: &mut bool,
) {
    let Command { request, reply } = command;
    match request {
        Request::Status {} => {
            let _ = reply.send(snapshot(manager, None));
        }
        Request::Start { targets, all } => {
            let result = if *shutting_down {
                Err("Workspace is shutting down.".into())
            } else {
                validate_selection(&targets, all).and_then(|()| {
                    if all {
                        manager.start_all()
                    } else {
                        manager.start_selected(&targets)
                    }
                })
            };
            let _ = reply.send(snapshot(manager, result.err()));
        }
        request => {
            if pending.len() >= 32 {
                let _ = reply.send(Response::failed("Too many pending stop requests."));
                return;
            }
            let ids = match request {
                Request::Shutdown {} => {
                    *shutting_down = true;
                    manager.request_stop_all();
                    manager.all_ids()
                }
                Request::Stop { targets, all } => {
                    if let Err(error) = validate_selection(&targets, all) {
                        let _ = reply.send(Response::failed(error));
                        return;
                    }
                    if all {
                        manager.request_stop_all();
                        manager.all_ids()
                    } else {
                        if let Err(error) = manager.stop_selected(&targets) {
                            let _ = reply.send(snapshot(manager, Some(error)));
                            return;
                        }
                        targets
                    }
                }
                _ => unreachable!(),
            };
            let errors = ids
                .iter()
                .filter_map(|id| {
                    let status = &manager.statuses()[id];
                    (status.state == ServiceState::Failed && !status.cleanup_confirmed).then(|| {
                        format!(
                            "Service '{id}': {}",
                            status.error.as_deref().unwrap_or("Cleanup unknown.")
                        )
                    })
                })
                .collect();
            pending.push(PendingStop { ids, errors, reply });
        }
    }
}

fn complete_stops(manager: &mut Manager, pending: &mut Vec<PendingStop>) {
    let mut index = 0;
    while index < pending.len() {
        if pending[index].ids.iter().any(|id| manager.is_pending(id)) {
            index += 1;
            continue;
        }
        let command = pending.swap_remove(index);
        let error = (!command.errors.is_empty()).then(|| command.errors.join("\n"));
        let _ = command.reply.send(snapshot(manager, error));
    }
}

pub async fn run(path: &Path, detached: bool) -> Result<(), String> {
    let result = run_inner(path, detached).await;
    if detached && let Err(error) = &result {
        startup::failed(error);
    }
    result
}

async fn run_inner(path: &Path, detached: bool) -> Result<(), String> {
    if detached {
        startup::detach()?;
    }
    let path = path
        .canonicalize()
        .map_err(|error| format!("Failed to resolve config path {}: {error}", path.display()))?;
    let config = config::load(&path)?;
    let mut interrupt = signal(SignalKind::interrupt())
        .map_err(|error| format!("Failed to register SIGINT handler: {error}"))?;
    let mut terminate = signal(SignalKind::terminate())
        .map_err(|error| format!("Failed to register SIGTERM handler: {error}"))?;
    let _run_lock = match RunLock::try_acquire(&path)? {
        Some(guard) => guard,
        None => {
            if detached {
                startup::locked();
            }
            return Err(format!("Config '{}' is already managed.", path.display()));
        }
    };
    let socket = ControlSocket::bind(&path, &_run_lock)?;
    let mut manager = Manager::new(config.services);
    let (commands, mut incoming) = mpsc::channel(32);
    let mut clients = JoinSet::new();
    let mut pending = Vec::<PendingStop>::new();
    let mut shutting_down = false;
    let mut reply_on_shutdown = false;
    if detached {
        startup::ready()?;
    }

    let outcome = loop {
        tokio::select! {
            _ = interrupt.recv(), if !shutting_down => {
                shutting_down = true;
                manager.request_stop_all();
            }
            _ = terminate.recv(), if !shutting_down => {
                shutting_down = true;
                manager.request_stop_all();
            }
            accepted = socket.accept(), if clients.len() < MAX_CLIENTS => {
                match accepted {
                    Ok(stream) => {
                        let config_path = path.clone();
                        let commands = commands.clone();
                        clients.spawn(async move { control::connection(stream, &config_path, commands).await });
                    }
                    Err(error) => break Err(format!("Failed to accept control connection: {error}")),
                }
            }
            Some(command) = incoming.recv() => {
                let shutdown_request = matches!(&command.request, Request::Shutdown {});
                dispatch(command, &mut manager, &mut pending, &mut shutting_down);
                reply_on_shutdown |= shutdown_request && shutting_down;
            }
            completion = manager.wait_next(), if manager.has_pending_tasks() => {
                if let Some((id, Err(error))) = completion {
                    for command in &mut pending {
                        if command.ids.contains(&id) { command.errors.push(format!("Service '{id}': {error}")); }
                    }
                }
            }
            Some(_result) = clients.join_next(), if !clients.is_empty() => {}
        }
        complete_stops(&mut manager, &mut pending);
        if shutting_down && !manager.has_pending_tasks() {
            break Ok(());
        }
    };

    drop(incoming);
    if !reply_on_shutdown
        || timeout(Duration::from_secs(3), async {
            while clients.join_next().await.is_some() {}
        })
        .await
        .is_err()
    {
        clients.shutdown().await;
    }
    match (outcome, manager.stop_all().await) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(error), Err(cleanup)) => Err(format!("{error}\n{cleanup}")),
    }
}
