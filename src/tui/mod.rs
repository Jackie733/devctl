mod app;
mod terminal;
mod view;

use std::{io::stdout, path::Path, time::Duration};

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures_util::StreamExt;
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::{
    signal::unix::{SignalKind, signal},
    task::{AbortHandle, Id, JoinSet},
    time::{MissedTickBehavior, interval},
};

use crate::{
    client::{self, ClientError},
    control::{Request, Response},
};
use app::{App, Operation};

enum Context {
    Status {
        epoch: u64,
    },
    Logs {
        epoch: u64,
        selection: u64,
        service: String,
    },
    Control {
        operation: Operation,
        workspace: String,
    },
}

struct Completed {
    context: Context,
    result: Result<Response, ClientError>,
}

#[derive(Default)]
struct Requests {
    tasks: JoinSet<Completed>,
    status: Option<AbortHandle>,
    logs: Option<AbortHandle>,
    control: Option<AbortHandle>,
}

impl Requests {
    fn spawn(&mut self, path: &Path, context: Context, request: Request, create: bool) {
        let slot = match &context {
            Context::Status { .. } => &mut self.status,
            Context::Logs { .. } => &mut self.logs,
            Context::Control { .. } => &mut self.control,
        };
        debug_assert!(slot.is_none());
        let path = path.to_owned();
        *slot = Some(self.tasks.spawn(async move {
            Completed {
                context,
                result: client::transact(&path, &request, create).await,
            }
        }));
    }

    fn clear(&mut self, id: Id) {
        for slot in [&mut self.status, &mut self.logs, &mut self.control] {
            if slot.as_ref().is_some_and(|handle| handle.id() == id) {
                *slot = None;
            }
        }
    }

    fn cancel_queries(&self) {
        for task in [&self.status, &self.logs].into_iter().flatten() {
            task.abort();
        }
        // Keep slots occupied until join: rapid input cannot grow the task set.
    }
}

enum Action {
    None,
    Redraw,
    Selection,
    Reconnect,
    Control(Operation),
    Quit,
}

fn handle_key(app: &mut App, key: KeyEvent, height: usize) -> Action {
    if key.kind != KeyEventKind::Press {
        return Action::None;
    }
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Action::Quit;
    }
    if key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
    {
        return Action::None;
    }
    if key.code == KeyCode::Char('q') {
        return Action::Quit;
    }
    if app.confirm_shutdown {
        return match key.code {
            KeyCode::Char('y') => {
                app.confirm_shutdown = false;
                Action::Control(Operation::Shutdown)
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                app.confirm_shutdown = false;
                Action::Redraw
            }
            _ => Action::None,
        };
    }
    if app.help {
        return match key.code {
            KeyCode::Char('?') | KeyCode::Esc => {
                app.help = false;
                Action::Redraw
            }
            _ => Action::None,
        };
    }
    match key.code {
        KeyCode::Up | KeyCode::Down => {
            if app.select(if key.code == KeyCode::Up { -1 } else { 1 }) {
                Action::Selection
            } else {
                Action::None
            }
        }
        KeyCode::PageUp => {
            app.logs
                .scroll(-((height.saturating_sub(1)).max(1) as isize), height);
            Action::Redraw
        }
        KeyCode::PageDown => {
            app.logs
                .scroll((height.saturating_sub(1)).max(1) as isize, height);
            Action::Redraw
        }
        KeyCode::Home => {
            app.logs.oldest();
            Action::Redraw
        }
        KeyCode::End => {
            app.logs.follow();
            Action::Redraw
        }
        KeyCode::Left => {
            app.logs.horizontal = app.logs.horizontal.saturating_sub(8);
            Action::Redraw
        }
        KeyCode::Right => {
            app.logs.horizontal = app.logs.horizontal.saturating_add(8);
            Action::Redraw
        }
        KeyCode::Char('r') => Action::Reconnect,
        KeyCode::Char('?') => {
            app.help = true;
            Action::Redraw
        }
        KeyCode::Char('Q') => {
            if app.connected && app.pending.is_none() {
                app.confirm_shutdown = true;
            } else {
                app.feedback =
                    "Reconnect and wait for any pending operation before ending the workspace."
                        .into();
            }
            Action::Redraw
        }
        KeyCode::Char('a') => Action::Control(Operation::StartAll),
        KeyCode::Char('X') => Action::Control(Operation::StopAll),
        KeyCode::Char('s') | KeyCode::Char('x') => match app.selected.clone() {
            Some(id) => Action::Control(if key.code == KeyCode::Char('s') {
                Operation::Start(id)
            } else {
                Operation::Stop(id)
            }),
            None => {
                app.feedback = "No project selected.".into();
                Action::Redraw
            }
        },
        _ => Action::None,
    }
}

pub async fn run(path: &Path) -> Result<(), String> {
    let mut interrupt = signal(SignalKind::interrupt()).map_err(|error| error.to_string())?;
    let mut terminate = signal(SignalKind::terminate()).map_err(|error| error.to_string())?;
    let mut hangup = signal(SignalKind::hangup()).map_err(|error| error.to_string())?;
    let mut guard = terminal::TerminalGuard::enter().map_err(|error| error.to_string())?;
    let mut requests = Requests::default();

    let result = async {
        let path = path.canonicalize().map_err(|error| format!("Failed to resolve config path {}: {error}", path.display()))?;
        let mut terminal = Terminal::new(CrosstermBackend::new(stdout())).map_err(|error| error.to_string())?;
        let mut events = EventStream::new();
        let mut app = App::new();
        let mut status_tick = interval(Duration::from_millis(500));
        let mut logs_tick = interval(Duration::from_millis(200));
        let mut draw_tick = interval(Duration::from_millis(33));
        for timer in [&mut status_tick, &mut logs_tick, &mut draw_tick] {
            timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
        }
        let mut create = true;
        let mut status_due = true;
        let mut logs_due = false;
        let mut dirty = true;
        loop {
            if status_due && requests.status.is_none() {
                requests.spawn(&path, Context::Status { epoch: app.epoch }, Request::Status {}, create);
                create = false; // Only this initial request may create an idle supervisor.
                status_due = false;
            }
            if logs_due && app.connected && requests.logs.is_none()
                && let Some(service) = app.selected.clone()
            {
                requests.spawn(&path, Context::Logs { epoch: app.epoch, selection: app.selection_epoch, service: service.clone() },
                    Request::Logs { service, limit: 100, after: app.logs.cursor }, false);
                logs_due = false;
            }
            tokio::select! {
                event = events.next() => {
                    let event = event.ok_or("Terminal event stream ended.")?.map_err(|error| error.to_string())?;
                    match event {
                        Event::Key(key) => {
                            let height = view::log_height(terminal.size().map_err(|error| error.to_string())?.height);
                            match handle_key(&mut app, key, height) {
                                Action::Quit => break Ok(()),
                                Action::None => {}
                                Action::Redraw => dirty = true,
                                Action::Selection => {
                                    if let Some(task) = &requests.logs { task.abort(); }
                                    logs_due = true;
                                    dirty = true;
                                }
                                Action::Reconnect => {
                                    app.disconnect("Reconnecting to the existing workspace...".into());
                                    requests.cancel_queries();
                                    status_due = true;
                                    dirty = true;
                                }
                                Action::Control(operation) => {
                                    if app.begin(operation.clone()) {
                                        requests.spawn(&path, Context::Control { operation: operation.clone(), workspace: app.workspace_id.clone().expect("Connected workspace has an identity.") }, operation.request(), false);
                                    }
                                    dirty = true;
                                }
                            }
                        }
                        Event::Resize(_, _) => dirty = true,
                        _ => {}
                    }
                }
                Some(completion) = requests.tasks.join_next_with_id(), if !requests.tasks.is_empty() => {
                    let (id, completed) = match completion {
                        Ok(value) => value,
                        Err(error) => {
                            requests.clear(error.id());
                            if error.is_cancelled() { continue; }
                            break Err(format!("Workspace request task failed: {error}"));
                        }
                    };
                    requests.clear(id);
                    let previous_epoch = app.epoch;
                    match completed.context {
                        Context::Status { epoch } => {
                            if epoch != app.epoch { continue; }
                            match completed.result {
                                Ok(response) if response.error.is_none() => {
                                    dirty |= app.apply_status(response);
                                    if app.epoch != previous_epoch { logs_due = true; }
                                }
                                Ok(response) => {
                                    app.disconnect(format!("Workspace status unavailable: {}", response.error.unwrap()));
                                    dirty = true;
                                }
                                Err(ClientError::Preparation(error)) if app.workspace_id.is_none() => break Err(error),
                                Err(error) => {
                                    app.disconnect(format!("Disconnected; state unknown. {error} Reopen TUI to create a new workspace if needed."));
                                    dirty = true;
                                }
                            }
                        }
                        Context::Logs { epoch, selection, service } => {
                            if !app.accepts_logs(epoch, selection, &service) { continue; }
                            match completed.result {
                                Ok(response) if app.workspace_id.as_ref() != Some(&response.workspace_id) => {
                                    app.disconnect("Workspace changed; refreshing state and recent logs.".into());
                                    status_due = true;
                                    dirty = true;
                                }
                                Ok(response) => {
                                    if let Some(error) = response.error {
                                        app.feedback = format!("Logs for {service}: {error}");
                                        dirty = true;
                                    } else if let Some(batch) = response.logs {
                                        dirty |= app.logs.append(batch);
                                    } else {
                                        app.disconnect("Missing log response; refreshing state.".into());
                                        status_due = true;
                                        dirty = true;
                                    }
                                }
                                Err(error) => {
                                    app.disconnect(format!("Disconnected; state unknown. {error}"));
                                    status_due = true;
                                    dirty = true;
                                }
                            }
                        }
                        Context::Control { operation, workspace } => {
                            match completed.result {
                                Ok(response) => {
                                    if app.control_response(&operation, &workspace, response) { break Ok(()); }
                                }
                                Err(error) => {
                                    app.pending = None;
                                    app.feedback = format!("{}: {error} Completion may be unknown; control requests are not replayed.", operation.label());
                                    app.disconnect("Disconnected; state unknown. Reconnecting without replaying control requests.".into());
                                }
                            }
                            status_due = true;
                            logs_due = true;
                            dirty = true;
                        }
                    }
                    if app.epoch != previous_epoch { requests.cancel_queries(); }
                }
                _ = status_tick.tick() => status_due = true,
                _ = logs_tick.tick() => logs_due = true,
                _ = draw_tick.tick() => {
                    if dirty {
                        terminal.draw(|frame| view::render(frame, &app)).map_err(|error| error.to_string())?;
                        dirty = false;
                    }
                }
                _ = interrupt.recv() => break Ok(()),
                _ = terminate.recv() => break Ok(()),
                _ = hangup.recv() => break Ok(()),
            }
        }
    }.await;

    // These tasks only own client connections, never service processes.
    requests.tasks.shutdown().await;
    match (result, guard.restore()) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(format!("Terminal restore failed: {error}")),
        (Err(error), Err(cleanup)) => Err(format!("{error}; terminal restore failed: {cleanup}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manager::ServiceStatus;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[tokio::test]
    async fn query_cancellation_keeps_capacity_bounded_and_does_not_cancel_control() {
        let mut requests = Requests::default();
        let path = std::env::temp_dir().join(format!("devctl-tui-missing-{}", std::process::id()));
        requests.spawn(
            &path,
            Context::Status { epoch: 0 },
            Request::Status {},
            false,
        );
        requests.spawn(
            &path,
            Context::Logs {
                epoch: 0,
                selection: 0,
                service: "web".into(),
            },
            Request::Logs {
                service: "web".into(),
                limit: 100,
                after: None,
            },
            false,
        );
        requests.spawn(
            &path,
            Context::Control {
                operation: Operation::StopAll,
                workspace: "one".into(),
            },
            Request::Stop {
                targets: Vec::new(),
                all: true,
            },
            false,
        );
        requests.cancel_queries();
        assert!(requests.status.is_some() && requests.logs.is_some() && requests.control.is_some());
        assert_eq!(requests.tasks.len(), 3);
        let mut cancelled = 0;
        let mut controls = 0;
        while let Some(result) = requests.tasks.join_next_with_id().await {
            match result {
                Ok((id, completed)) => {
                    requests.clear(id);
                    assert!(matches!(completed.context, Context::Control { .. }));
                    controls += 1;
                }
                Err(error) => {
                    requests.clear(error.id());
                    assert!(error.is_cancelled());
                    cancelled += 1;
                }
            }
        }
        assert_eq!((cancelled, controls), (2, 1));
        assert!(requests.status.is_none() && requests.logs.is_none() && requests.control.is_none());
    }

    #[test]
    fn shutdown_requires_confirmation_and_leaving_never_becomes_shutdown() {
        let mut app = App::new();
        app.apply_status(Response {
            workspace_id: "one".into(),
            services: [("web".into(), ServiceStatus::default())].into(),
            ..Response::default()
        });
        assert!(matches!(
            handle_key(&mut app, key(KeyCode::Char('y')), 10),
            Action::None
        ));
        assert!(matches!(
            handle_key(&mut app, key(KeyCode::Char('Q')), 10),
            Action::Redraw
        ));
        assert!(app.confirm_shutdown);
        assert!(matches!(
            handle_key(&mut app, key(KeyCode::Char('s')), 10),
            Action::None
        ));
        assert!(matches!(
            handle_key(&mut app, key(KeyCode::Char('q')), 10),
            Action::Quit
        ));
        assert!(matches!(
            handle_key(
                &mut app,
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                10
            ),
            Action::Quit
        ));
        assert!(matches!(
            handle_key(&mut app, key(KeyCode::Char('y')), 10),
            Action::Control(Operation::Shutdown)
        ));
        assert!(!app.confirm_shutdown);
        handle_key(&mut app, key(KeyCode::Char('Q')), 10);
        handle_key(&mut app, key(KeyCode::Esc), 10);
        assert!(!app.confirm_shutdown && app.pending.is_none());
    }
}
