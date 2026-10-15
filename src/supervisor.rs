use std::{io::Read, path::Path, time::Duration};

use tokio::{
    signal::unix::{SignalKind, signal},
    sync::{mpsc, oneshot},
    task::JoinSet,
    time::{Instant, sleep_until, timeout},
};

use crate::{
    config,
    control::{self, Command, Request, Response},
    control_socket::ControlSocket,
    logs::{self, Store},
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
        ..Response::default()
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
    logs: &Store,
) {
    let Command { request, reply } = command;
    match request {
        Request::Logs {
            service,
            limit,
            after,
            before,
        } => {
            let response = match logs.query(&service, limit, after, before) {
                Ok(logs) => Response {
                    logs: Some(logs),
                    ..Response::default()
                },
                Err(error) => Response::failed(error),
            };
            let _ = reply.send(response);
        }
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

fn workspace_identity() -> Result<String, String> {
    let mut bytes = [0; 16];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|error| format!("Failed to create workspace instance identity: {error}"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
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
    let workspace_id = workspace_identity()?;
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
    let (sinks, mut output, mut logs) = logs::channel(config.services.keys().cloned());
    let mut manager = Manager::with_logs(config.services, sinks);
    let (commands, mut incoming) = mpsc::channel(32);
    let mut clients = JoinSet::new();
    let mut pending = Vec::<PendingStop>::new();
    let mut shutting_down = false;
    let mut reply_on_shutdown = false;
    if detached {
        startup::ready()?;
    }

    let mut accept_after = Instant::now();
    loop {
        let accepting = Instant::now() >= accept_after;
        tokio::select! {
            _ = interrupt.recv(), if !shutting_down => {
                shutting_down = true;
                manager.request_stop_all();
            }
            _ = terminate.recv(), if !shutting_down => {
                shutting_down = true;
                manager.request_stop_all();
            }
            accepted = socket.accept(), if accepting && clients.len() < MAX_CLIENTS => {
                match accepted {
                    Ok(stream) => {
                        let config_path = path.clone();
                        let commands = commands.clone();
                        let workspace_id = workspace_id.clone();
                        clients.spawn(async move { control::connection(stream, &config_path, commands, &workspace_id).await });
                    }
                    Err(error) => {
                        eprintln!("Failed to accept control connection: {error}");
                        accept_after = Instant::now() + Duration::from_millis(100);
                    }
                }
            }
            _ = sleep_until(accept_after), if !accepting => {}
            Some(command) = incoming.recv() => {
                let shutdown_request = matches!(&command.request, Request::Shutdown {});
                dispatch(command, &mut manager, &mut pending, &mut shutting_down, &logs);
                reply_on_shutdown |= shutdown_request && shutting_down;
            }
            Some(event) = output.recv() => logs.push(event),
            completion = manager.wait_next(), if manager.has_pending_tasks() => {
                // Drain at most one queue capacity before acknowledging completed stops.
                for _ in 0..logs::QUEUE_CAPACITY {
                    match output.try_recv() { Ok(event) => logs.push(event), Err(_) => break }
                }
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
            if manager.has_unconfirmed_cleanup() {
                // Keep the ownership lock and diagnostics when safe exit cannot be proved.
                shutting_down = false;
                reply_on_shutdown = false;
            } else {
                break;
            }
        }
    }

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
    manager.stop_all().await
}
