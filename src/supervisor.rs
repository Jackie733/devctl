use std::{io::Read, path::Path, sync::Arc, time::Duration};

use tokio::{
    signal::unix::{SignalKind, signal},
    sync::{mpsc, oneshot},
    task::JoinSet,
    time::{Instant, sleep_until, timeout},
};

use crate::{
    archive::{Archive, Query},
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
    shutdown: bool,
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
    archive: &Archive,
) {
    let Command { request, reply } = command;
    match request {
        Request::Logs {
            service,
            limit,
            after,
            before,
        } => {
            let query = Query {
                service: service.clone(),
                limit,
                after,
                before,
                session: None,
                contains: None,
                upper: None,
            };
            match logs.query(&service, limit, after, before) {
                Ok(memory) => archive.query(query, memory, reply),
                Err(error) => {
                    let _ = reply.send(Response::failed(error));
                }
            }
        }
        Request::ReadLogs { query } => {
            match logs.query_matching(
                &query.service,
                query.limit,
                query.after,
                query.before,
                query.contains.as_deref(),
                query.upper,
            ) {
                Ok(memory) => archive.query(query, memory, reply),
                Err(_) if query.session.is_some() => {
                    archive.query(query, logs::Batch::default(), reply)
                }
                Err(error) => {
                    let _ = reply.send(Response::failed(error));
                }
            }
        }
        Request::LogSessions {} => archive.list(reply),
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
            let shutdown = matches!(request, Request::Shutdown {});
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
            pending.push(PendingStop {
                shutdown,
                ids,
                errors,
                reply,
            });
        }
    }
}

fn complete_stops(manager: &mut Manager, pending: &mut Vec<PendingStop>, allow_shutdown: bool) {
    let mut index = 0;
    while index < pending.len() {
        if (!allow_shutdown && pending[index].shutdown)
            || pending[index].ids.iter().any(|id| manager.is_pending(id))
        {
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
        Some(guard) => Arc::new(guard),
        None => {
            if detached {
                startup::locked();
            }
            return Err(format!("Config '{}' is already managed.", path.display()));
        }
    };
    let socket = ControlSocket::bind(&path, &_run_lock)?;
    let (sinks, mut output, mut logs) = logs::channel(config.services.keys().cloned());
    let archive = Archive::start(&path, config.logs, workspace_id.clone(), _run_lock.clone())?;
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
                dispatch(command, &mut manager, &mut pending, &mut shutting_down, &logs, &archive);
                reply_on_shutdown |= shutdown_request && shutting_down;
            }
            Some(event) = output.recv() => { if let Some((service, record)) = logs.push(event) {archive.push(service,record);} },
            completion = manager.wait_next(), if manager.has_pending_tasks() => {
                // Drain at most one queue capacity before acknowledging completed stops.
                for _ in 0..logs::QUEUE_CAPACITY {
                    match output.try_recv() { Ok(event) => {if let Some((service,record)) = logs.push(event) {archive.push(service,record);}}, Err(_) => break }
                }
                if let Some((id, Err(error))) = completion {
                    for command in &mut pending {
                        if command.ids.contains(&id) { command.errors.push(format!("Service '{id}': {error}")); }
                    }
                }
            }
            Some(_result) = clients.join_next(), if !clients.is_empty() => {}
        }
        complete_stops(&mut manager, &mut pending, !shutting_down);
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
    let stopped = manager.stop_all().await;
    for _ in 0..logs::QUEUE_CAPACITY {
        match output.try_recv() {
            Ok(event) => {
                if let Some((service, record)) = logs.push(event) {
                    archive.push(service, record);
                }
            }
            Err(_) => break,
        }
    }
    let incomplete = manager.all_ids().iter().any(|id| {
        logs.query(id, 1, None, None)
            .is_ok_and(|b| b.incomplete || b.dropped > b.evicted)
    });
    let saved = archive.finish(incomplete).await;
    if let Err(error) = &saved {
        for command in &mut pending {
            if command.shutdown {
                command.errors.push(error.clone());
            }
        }
    }
    complete_stops(&mut manager, &mut pending, true);
    if !reply_on_shutdown
        || timeout(Duration::from_secs(3), async {
            while clients.join_next().await.is_some() {}
        })
        .await
        .is_err()
    {
        clients.shutdown().await;
    }
    stopped.and(saved)
}
