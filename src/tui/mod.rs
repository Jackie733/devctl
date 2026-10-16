mod app;
mod terminal;
mod view;

use std::{
    io::stdout,
    path::Path,
    time::{Duration, Instant},
};

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
use app::{App, Focus, LogQuery, Operation};

enum Context {
    Status {
        epoch: u64,
    },
    Logs {
        epoch: u64,
        selection: u64,
        service: String,
        query: LogQuery,
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
    Search,
    Reconnect,
    Control(Operation),
    Copy,
    Quit,
}

fn handle_key(app: &mut App, key: KeyEvent, area: ratatui::layout::Rect) -> Action {
    if key.kind != KeyEventKind::Press {
        return Action::None;
    }
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Action::Quit;
    }
    let half_page =
        key.modifiers == KeyModifiers::CONTROL && matches!(key.code, KeyCode::Char('u' | 'd'));
    if !half_page
        && key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
    {
        return Action::None;
    }
    if let Some(input) = &mut app.search_input {
        match key.code {
            KeyCode::Esc => {
                app.search_input = None;
                app.feedback = "Search input cancelled.".into();
            }
            KeyCode::Backspace => {
                input.pop();
            }
            KeyCode::Char(c) if input.len() + c.len_utf8() <= 256 && !c.is_control() => {
                input.push(c)
            }
            KeyCode::Enter => {
                let text = input.clone();
                app.search_input = None;
                if !text.is_empty() {
                    app.logs
                        .start_search(text, false, true, view::log_height(area, app));
                    app.feedback = "Searching retained history... Esc cancels.".into();
                    return Action::Search;
                }
            }
            _ => {}
        }
        return Action::Redraw;
    }
    if key.code == KeyCode::Char('q') {
        return Action::Quit;
    }
    if !view::usable(area) {
        if key.code == KeyCode::Esc || key.code == KeyCode::Char('n') {
            app.confirm_shutdown = false;
            app.goto_top = false;
        }
        return Action::Redraw;
    }
    if app.confirm_shutdown {
        return match key.code {
            KeyCode::Char('y')
                if app.connected && app.pending.is_none() && view::can_confirm(area, app) =>
            {
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
    if app.goto_top && key.code == KeyCode::Esc {
        app.goto_top = false;
        return Action::Redraw;
    }
    let prefix = app.goto_top;
    let top = prefix && key.code == KeyCode::Char('g');
    app.goto_top = false;
    if key.code == KeyCode::Char('g') && !top {
        app.goto_top = true;
        return Action::Redraw;
    }
    if app.help {
        let max = view::help_max_scroll(area, app);
        app.help_scroll = app.help_scroll.min(max);
        match key.code {
            KeyCode::Char('?') | KeyCode::Esc => app.help = false,
            KeyCode::Char('g') if top => app.help_scroll = 0,
            KeyCode::Char('G') => app.help_scroll = max,
            KeyCode::Char('j') | KeyCode::Down => {
                app.help_scroll = app.help_scroll.saturating_add(1).min(max)
            }
            KeyCode::Char('k') | KeyCode::Up => app.help_scroll = app.help_scroll.saturating_sub(1),
            KeyCode::PageDown => {
                app.help_scroll = app
                    .help_scroll
                    .saturating_add(view::help_height(area, app))
                    .min(max)
            }
            KeyCode::PageUp => {
                app.help_scroll = app.help_scroll.saturating_sub(view::help_height(area, app))
            }
            _ => {}
        }
        return Action::Redraw;
    }
    let height = view::log_height(area, app);
    if app.logs.selection.is_some() {
        match key.code {
            KeyCode::Esc | KeyCode::Char('v' | 'V') => app.logs.selection = None,
            KeyCode::Char('y') => return Action::Copy,
            KeyCode::Char('?') => {
                app.help = true;
                app.help_scroll = 0;
            }
            KeyCode::Char('g') if top => app.logs.move_selection(isize::MIN, height),
            KeyCode::Char('G') => app.logs.move_selection(isize::MAX, height),
            KeyCode::Char('j') | KeyCode::Down => app.logs.move_selection(1, height),
            KeyCode::Char('k') | KeyCode::Up => app.logs.move_selection(-1, height),
            KeyCode::Char('u') if half_page => app
                .logs
                .move_selection(-((height / 2).max(1) as isize), height),
            KeyCode::Char('d') if half_page => app
                .logs
                .move_selection((height / 2).max(1) as isize, height),
            KeyCode::Home => app.logs.move_selection(isize::MIN, height),
            KeyCode::End => app.logs.move_selection(isize::MAX, height),
            KeyCode::PageUp => app
                .logs
                .move_selection(-(height.saturating_sub(1).max(1) as isize), height),
            KeyCode::PageDown => app
                .logs
                .move_selection(height.saturating_sub(1).max(1) as isize, height),
            KeyCode::Left | KeyCode::Char('H') => {
                app.logs.horizontal = app.logs.horizontal.saturating_sub(8)
            }
            KeyCode::Right | KeyCode::Char('L') => {
                app.logs.horizontal = app.logs.horizontal.saturating_add(8)
            }
            _ => {}
        }
        return Action::Redraw;
    }
    if key.code == KeyCode::Esc && app.logs.search.is_some() && app.focus == Focus::Logs {
        app.logs.clear_search();
        app.feedback = "Search cleared; reading position kept.".into();
        return Action::Search;
    }
    if half_page {
        let direction = if key.code == KeyCode::Char('u') {
            -1
        } else {
            1
        };
        app.logs
            .scroll(direction * (height / 2).max(1) as isize, height);
        return Action::Redraw;
    }
    let direction = match key.code {
        KeyCode::Char('j') | KeyCode::Down => Some(1),
        KeyCode::Char('k') | KeyCode::Up => Some(-1),
        KeyCode::Char('g') if top => Some(isize::MIN),
        KeyCode::Char('G') => Some(isize::MAX),
        _ => None,
    };
    if let Some(direction) = direction {
        if app.focus == Focus::Projects {
            return if app.select(direction) {
                Action::Selection
            } else {
                Action::Redraw
            };
        }
        match key.code {
            KeyCode::Char('g') => app.logs.oldest(),
            KeyCode::Char('G') => app.logs.follow(),
            _ => app.logs.move_reading(direction, height),
        }
        return Action::Redraw;
    }
    match key.code {
        KeyCode::Char('h' | '1') | KeyCode::Esc => app.focus(Focus::Projects),
        KeyCode::Char('l' | '0') | KeyCode::Enter => app.focus(Focus::Logs),
        KeyCode::Tab | KeyCode::BackTab => app.focus(if app.focus == Focus::Projects {
            Focus::Logs
        } else {
            Focus::Projects
        }),
        KeyCode::PageUp => app
            .logs
            .scroll(-(height.saturating_sub(1).max(1) as isize), height),
        KeyCode::PageDown => app
            .logs
            .scroll(height.saturating_sub(1).max(1) as isize, height),
        KeyCode::Home => app.logs.oldest(),
        KeyCode::End => app.logs.follow(),
        KeyCode::Char('p') => {
            if app.logs.paused {
                app.logs.follow();
            } else {
                app.logs.pause(height);
            }
        }
        KeyCode::Left | KeyCode::Char('H') => {
            app.logs.horizontal = app.logs.horizontal.saturating_sub(8)
        }
        KeyCode::Right | KeyCode::Char('L') => {
            app.logs.horizontal = app.logs.horizontal.saturating_add(8)
        }
        KeyCode::Char('/') if app.focus == Focus::Logs => {
            app.logs.cancel_search();
            app.search_input = Some(String::new());
            return Action::Search;
        }
        KeyCode::Char('n' | 'N') if app.focus == Focus::Logs => {
            if let Some(search) = &app.logs.search {
                let text = search.text.clone();
                app.logs
                    .start_search(text, key.code == KeyCode::Char('N'), false, height);
                app.feedback = "Searching retained history... Esc cancels.".into();
                return Action::Search;
            } else {
                app.feedback = "Use / to enter search text first.".into();
            }
        }
        KeyCode::Char('v' | 'V') if app.focus == Focus::Logs => {
            if !app.logs.begin_selection(height) {
                app.feedback = "No cached output to select.".into();
            }
        }
        KeyCode::Char('r') => return Action::Reconnect,
        KeyCode::Char('?') => {
            app.help = true;
            app.help_scroll = 0;
        }
        KeyCode::Char('Q') => {
            if app.connected && app.pending.is_none() {
                app.confirm_shutdown = true;
            } else {
                app.feedback =
                    "Reconnect and wait for any pending operation before ending the workspace."
                        .into();
            }
        }
        KeyCode::Char('a') => return Action::Control(Operation::StartAll),
        KeyCode::Char('X') => return Action::Control(Operation::StopAll),
        KeyCode::Char('s' | 'x') => match app.selected.clone() {
            Some(id) => {
                return Action::Control(if key.code == KeyCode::Char('s') {
                    Operation::Start(id)
                } else {
                    Operation::Stop(id)
                });
            }
            None => app.feedback = "No project selected.".into(),
        },
        _ => return if prefix { Action::Redraw } else { Action::None },
    }
    Action::Redraw
}

pub async fn run(path: &Path) -> Result<(), String> {
    let mut interrupt = signal(SignalKind::interrupt()).map_err(|error| error.to_string())?;
    let mut terminate = signal(SignalKind::terminate()).map_err(|error| error.to_string())?;
    let mut hangup = signal(SignalKind::hangup()).map_err(|error| error.to_string())?;
    let mut guard = terminal::TerminalGuard::enter().map_err(|error| error.to_string())?;
    let mut requests = Requests::default();
    let mut copies = JoinSet::new();

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
                let query = app.logs.query();
                requests.spawn(&path, Context::Logs { epoch: app.epoch, selection: app.selection_epoch, service: service.clone(), query },
                    query.request(service,app.logs.search.as_ref()), false);
                logs_due = false;
            }
            tokio::select! {
                event = events.next() => {
                    let event = event.ok_or("Terminal event stream ended.")?.map_err(|error| error.to_string())?;
                    match event {
                        Event::Key(key) => {
                            let size = terminal.size().map_err(|error| error.to_string())?;
                            let area = ratatui::layout::Rect::new(0, 0, size.width, size.height);
                            let previous_logs = (app.logs.paused, app.logs.requested);
                            match handle_key(&mut app, key, area) {
                                Action::Quit => break Ok(()),
                                Action::None => {}
                                Action::Redraw => dirty = true,
                                Action::Search => {
                                    app.selection_epoch += 1;
                                    if let Some(task) = &requests.logs { task.abort(); }
                                    logs_due = true;
                                    dirty = true;
                                }
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
                                Action::Copy => {
                                    if !copies.is_empty() {
                                        app.feedback = "Clipboard copy in progress; wait before copying again.".into();
                                        dirty = true;
                                        continue;
                                    }
                                    match app.logs.selected_text() {
                                        Ok((text, count)) => {
                                            let context = (app.epoch, app.selection_epoch, app.logs.selection);
                                            copies.spawn(async move { (context, count, terminal::copy_logs(&text).await) });
                                            app.copying = true;
                                            app.feedback = format!("Copying {count} log lines...");
                                        }
                                        Err(error) => app.feedback = error.into(),
                                    }
                                    dirty = true;
                                }
                                Action::Control(operation) => {
                                    if app.begin(operation.clone()) {
                                        requests.spawn(&path, Context::Control { operation: operation.clone(), workspace: app.workspace_id.clone().expect("Connected workspace has an identity.") }, operation.request(), false);
                                    }
                                    dirty = true;
                                }
                            }
                            if previous_logs != (app.logs.paused, app.logs.requested) {
                                // A tail/pause/history transition invalidates the previous query.
                                app.selection_epoch += 1;
                                if let Some(task) = &requests.logs { task.abort(); }
                                logs_due = true;
                            }
                        }
                        Event::Resize(width, height) => {
                            let area = ratatui::layout::Rect::new(0, 0, width, height);
                            let height = view::log_height(area, &app);
                            app.logs.keep_visible(height);
                            dirty = true;
                        },
                        _ => {}
                    }
                }
                Some(completion) = copies.join_next(), if !copies.is_empty() => {
                    app.copying = false;
                    match completion {
                        Ok((context, count, result)) => {
                            let unchanged = context == (app.epoch, app.selection_epoch, app.logs.selection);
                            match result {
                                Ok(target) => {
                                    if unchanged { app.logs.selection = None; }
                                    let feedback = match target {
                                        terminal::CopyTarget::Windows => format!("Copied {count} lines via clip.exe; verify paste."),
                                        terminal::CopyTarget::Mac => format!("Copied {count} lines via pbcopy; verify paste."),
                                        terminal::CopyTarget::Terminal => format!("Sent {count} lines via OSC 52; terminal support required; verify paste."),
                                    };
                                    app.success(feedback);
                                }
                                Err(error) => app.feedback = format!("Clipboard copy failed: {error}. Select again or retry y."),
                            }
                        }
                        Err(error) => app.feedback = format!("Clipboard task failed: {error}"),
                    }
                    dirty = true;
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
                        Context::Logs { epoch, selection, service, query } => {
                            if !app.accepts_logs(epoch, selection, &service) { continue; }
                            match completed.result {
                                Ok(response) if app.workspace_id.as_ref() != Some(&response.workspace_id) => {
                                    app.disconnect("Workspace changed; refreshing state and recent logs.".into());
                                    status_due = true;
                                    dirty = true;
                                }
                                Ok(response) => {
                                    if let Some(error) = response.error {
                                        app.logs_error = Some(format!("{service}: {error}"));
                                        app.logs.cancel_search();
                                        dirty = true;
                                    } else if let Some(batch) = response.logs {
                                        let had_records = !batch.records.is_empty();
                                        dirty |= app.apply_log_page(query, batch);
                                        if matches!(query,LogQuery::Search(_) | LogQuery::Context(_)) || (matches!(query,LogQuery::ContextAfter(_)) && had_records) {
                                            let size = terminal.size().map_err(|e| e.to_string())?;
                                            view::reveal_search_match(&mut app,ratatui::layout::Rect::new(0,0,size.width,size.height));
                                        }
                                        logs_due |= app.logs.requested.is_some();
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
                    dirty |= app.tick(Instant::now());
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
    copies.shutdown().await;
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

    fn area() -> ratatui::layout::Rect {
        ratatui::layout::Rect::new(0, 0, 120, 30)
    }

    fn connected() -> App {
        let mut app = App::new();
        app.apply_status(Response {
            workspace_id: "one".into(),
            services: ["a", "b", "c"]
                .into_iter()
                .map(|id| (id.into(), ServiceStatus::default()))
                .collect(),
            ..Response::default()
        });
        app.apply_logs(crate::logs::Batch {
            records: (1..=100)
                .map(|sequence| crate::logs::Record {
                    captured_ms: 0,
                    sequence,
                    instance: 1,
                    source: crate::logs::Source::Stdout,
                    text: format!("line {sequence}"),
                    truncated: false,
                })
                .collect(),
            ..crate::logs::Batch::default()
        });
        app
    }

    #[test]
    fn focus_changes_preserve_selection_queries_and_log_position() {
        let mut app = connected();
        app.logs.oldest();
        let original = (
            app.selected.clone(),
            app.epoch,
            app.selection_epoch,
            app.logs.top,
            app.logs.cursor,
        );
        for code in [
            KeyCode::Char('l'),
            KeyCode::Char('h'),
            KeyCode::Tab,
            KeyCode::BackTab,
            KeyCode::Char('0'),
            KeyCode::Esc,
            KeyCode::Enter,
            KeyCode::Char('1'),
        ] {
            assert!(matches!(
                handle_key(&mut app, key(code), area()),
                Action::Redraw
            ));
            assert_eq!(
                (
                    app.selected.clone(),
                    app.epoch,
                    app.selection_epoch,
                    app.logs.top,
                    app.logs.cursor
                ),
                original
            );
        }
        handle_key(&mut app, key(KeyCode::Enter), area());
        handle_key(&mut app, key(KeyCode::Char('j')), area());
        assert_eq!(app.logs.top, Some(1));
        assert_eq!(app.logs.reading, Some(2));
        assert_eq!(app.selected.as_deref(), Some("a"));
        handle_key(&mut app, key(KeyCode::Esc), area());
        assert!(matches!(
            handle_key(&mut app, key(KeyCode::Char('j')), area()),
            Action::Selection
        ));
        assert_eq!(app.selected.as_deref(), Some("b"));
        assert!(app.logs.records.is_empty());
        assert!(!app.logs_loaded);
    }

    #[test]
    fn vim_prefix_is_bounded_cancelable_and_does_not_swallow_controls() {
        let mut app = connected();
        handle_key(&mut app, key(KeyCode::Char('G')), area());
        assert_eq!(app.selected.as_deref(), Some("c"));
        handle_key(&mut app, key(KeyCode::Char('g')), area());
        assert!(app.goto_top);
        assert_eq!(app.selected.as_deref(), Some("c"));
        handle_key(&mut app, key(KeyCode::Esc), area());
        assert!(!app.goto_top);
        handle_key(&mut app, key(KeyCode::Char('g')), area());
        assert!(
            matches!(handle_key(&mut app, key(KeyCode::Char('x')), area()), Action::Control(Operation::Stop(id)) if id == "c")
        );
        assert!(!app.goto_top);
        handle_key(&mut app, key(KeyCode::Char('g')), area());
        handle_key(&mut app, key(KeyCode::Char('g')), area());
        assert_eq!(app.selected.as_deref(), Some("a"));
        handle_key(&mut app, key(KeyCode::Char('g')), area());
        app.disconnect("lost".into());
        assert!(!app.goto_top);
    }

    #[test]
    fn pause_is_independent_of_focus_and_does_not_escape_selection_or_help() {
        let mut app = connected();
        let selected = app.selected.clone();
        handle_key(&mut app, key(KeyCode::Char('p')), area());
        assert!(app.logs.paused);
        assert_eq!(app.logs.query(), LogQuery::Bounds);
        assert_eq!(app.focus, Focus::Projects);
        assert_eq!(app.selected, selected);
        app.focus(Focus::Logs);
        app.logs.begin_selection(10);
        let selection = app.logs.selection;
        handle_key(&mut app, key(KeyCode::Char('p')), area());
        assert!(app.logs.paused);
        assert_eq!(app.logs.selection, selection);
        app.logs.selection = None;
        app.help = true;
        handle_key(&mut app, key(KeyCode::Char('p')), area());
        assert!(app.logs.paused);
        app.help = false;
        handle_key(&mut app, key(KeyCode::Char('p')), area());
        assert!(!app.logs.paused);
        assert_eq!(app.logs.query(), LogQuery::Tail);
    }

    #[test]
    fn log_navigation_uses_actual_viewport_and_preserves_legacy_keys() {
        let mut app = connected();
        let height = view::log_height(area(), &app);
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
            area(),
        );
        assert_eq!(app.logs.start(height), 100 - height - height / 2);
        assert_eq!(app.focus, Focus::Projects);
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
            area(),
        );
        assert_eq!(app.logs.top, None);
        handle_key(&mut app, key(KeyCode::Home), area());
        assert_eq!(app.logs.top, Some(1));
        handle_key(&mut app, key(KeyCode::PageDown), area());
        assert_eq!(app.logs.start(height), height - 1);
        handle_key(&mut app, key(KeyCode::Right), area());
        handle_key(&mut app, key(KeyCode::Char('L')), area());
        assert_eq!(app.logs.horizontal, 16);
        handle_key(&mut app, key(KeyCode::Char('H')), area());
        assert_eq!(app.logs.horizontal, 8);
        handle_key(&mut app, key(KeyCode::End), area());
        assert_eq!(app.logs.top, None);
        handle_key(&mut app, key(KeyCode::Char('0')), area());
        handle_key(&mut app, key(KeyCode::Char('g')), area());
        handle_key(&mut app, key(KeyCode::Char('g')), area());
        assert_eq!(app.logs.top, Some(1));
        handle_key(&mut app, key(KeyCode::Char('G')), area());
        assert_eq!(app.logs.top, None);
    }

    #[test]
    fn help_paging_uses_the_popup_body_and_clamps_at_both_ends() {
        let mut app = connected();
        let area = ratatui::layout::Rect::new(0, 0, 60, 16);
        handle_key(&mut app, key(KeyCode::Char('?')), area);
        let page = view::help_height(area, &app);
        assert!(view::help_max_scroll(area, &app) > page);
        handle_key(&mut app, key(KeyCode::PageDown), area);
        assert_eq!(app.help_scroll, page);
        handle_key(&mut app, key(KeyCode::PageUp), area);
        assert_eq!(app.help_scroll, 0);
        handle_key(&mut app, key(KeyCode::PageUp), area);
        assert_eq!(app.help_scroll, 0);
        handle_key(&mut app, key(KeyCode::Char('G')), area);
        handle_key(&mut app, key(KeyCode::PageDown), area);
        assert_eq!(app.help_scroll, view::help_max_scroll(area, &app));
    }

    #[test]
    fn help_consumes_commands_and_scrolls_without_moving_underlying_panes() {
        let mut app = connected();
        app.logs.oldest();
        app.focus(Focus::Logs);
        handle_key(&mut app, key(KeyCode::Char('?')), area());
        for code in ['s', 'x', 'a', 'X', 'Q', 'r'] {
            assert!(matches!(
                handle_key(&mut app, key(KeyCode::Char(code)), area()),
                Action::Redraw
            ));
        }
        handle_key(&mut app, key(KeyCode::Char('G')), area());
        assert_eq!(app.help_scroll, view::help_max_scroll(area(), &app));
        handle_key(&mut app, key(KeyCode::Char('g')), area());
        handle_key(&mut app, key(KeyCode::Char('g')), area());
        assert_eq!(app.help_scroll, 0);
        assert!(!app.confirm_shutdown);
        assert_eq!(app.logs.top, Some(1));
        assert_eq!(app.selected.as_deref(), Some("a"));
        handle_key(&mut app, key(KeyCode::Esc), area());
        assert!(!app.help);
        assert_eq!(app.focus, Focus::Logs);
        handle_key(&mut app, key(KeyCode::Char('?')), area());
        assert!(matches!(
            handle_key(&mut app, key(KeyCode::Char('q')), area()),
            Action::Quit
        ));
    }

    #[test]
    fn tiny_windows_and_release_events_cannot_control_services() {
        let mut app = connected();
        let tiny = ratatui::layout::Rect::new(0, 0, 20, 6);
        for code in ['s', 'x', 'a', 'X', 'Q', 'y'] {
            assert!(!matches!(
                handle_key(&mut app, key(KeyCode::Char(code)), tiny),
                Action::Control(_)
            ));
        }
        handle_key(&mut app, key(KeyCode::Char('Q')), area());
        assert!(app.confirm_shutdown);
        assert!(!matches!(
            handle_key(&mut app, key(KeyCode::Char('y')), tiny),
            Action::Control(_)
        ));
        assert!(matches!(
            handle_key(&mut app, key(KeyCode::Char('q')), tiny),
            Action::Quit
        ));
        let mut release = key(KeyCode::Char('y'));
        release.kind = KeyEventKind::Release;
        assert!(matches!(
            handle_key(&mut app, release, area()),
            Action::None
        ));
        app.disconnect("lost".into());
        assert!(!app.confirm_shutdown);
        assert!(!app.begin(Operation::Start("a".into())));
        let mut empty = App::new();
        assert!(!matches!(
            handle_key(&mut empty, key(KeyCode::Char('s')), area()),
            Action::Control(_)
        ));
    }

    #[test]
    fn visual_line_mode_is_modal_and_escape_stays_in_logs() {
        for code in ['v', 'V'] {
            let mut app = connected();
            app.focus(Focus::Logs);
            handle_key(&mut app, key(KeyCode::Char(code)), area());
            let anchor = app.logs.selection.unwrap().anchor;
            handle_key(&mut app, key(KeyCode::Char('j')), area());
            assert_eq!(app.logs.selection.unwrap().head, anchor + 1);
            for code in ['s', 'x', 'a', 'X', 'Q', 'r', 'h', '1'] {
                assert!(matches!(
                    handle_key(&mut app, key(KeyCode::Char(code)), area()),
                    Action::Redraw
                ));
            }
            assert_eq!(app.focus, Focus::Logs);
            assert!(!app.confirm_shutdown);
            assert!(matches!(
                handle_key(&mut app, key(KeyCode::Char('y')), area()),
                Action::Copy
            ));
            handle_key(&mut app, key(KeyCode::Char('?')), area());
            assert!(matches!(
                handle_key(&mut app, key(KeyCode::Char('y')), area()),
                Action::Redraw
            ));
            handle_key(&mut app, key(KeyCode::Esc), area());
            assert!(app.logs.selection.is_some());
            handle_key(&mut app, key(KeyCode::Char(code)), area());
            assert!(app.logs.selection.is_none());
            handle_key(&mut app, key(KeyCode::Char(code)), area());
            assert!(app.logs.selection.is_some());
            handle_key(&mut app, key(KeyCode::Esc), area());
            assert!(app.logs.selection.is_none());
            assert_eq!(app.focus, Focus::Logs);
            assert!(app.logs.top.is_some());
            handle_key(&mut app, key(KeyCode::Esc), area());
            assert_eq!(app.focus, Focus::Projects);
            assert!(!matches!(
                handle_key(&mut app, key(KeyCode::Char('y')), area()),
                Action::Copy
            ));
            handle_key(&mut app, key(KeyCode::Char(code)), area());
            assert!(app.logs.selection.is_none());
        }
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
                query: LogQuery::Tail,
            },
            Request::Logs {
                service: "web".into(),
                limit: 100,
                after: None,
                before: None,
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
            handle_key(
                &mut app,
                key(KeyCode::Char('y')),
                ratatui::layout::Rect::new(0, 0, 120, 24)
            ),
            Action::None
        ));
        assert!(matches!(
            handle_key(
                &mut app,
                key(KeyCode::Char('Q')),
                ratatui::layout::Rect::new(0, 0, 120, 24)
            ),
            Action::Redraw
        ));
        assert!(app.confirm_shutdown);
        assert!(matches!(
            handle_key(
                &mut app,
                key(KeyCode::Char('s')),
                ratatui::layout::Rect::new(0, 0, 120, 24)
            ),
            Action::None
        ));
        assert!(matches!(
            handle_key(
                &mut app,
                key(KeyCode::Char('q')),
                ratatui::layout::Rect::new(0, 0, 120, 24)
            ),
            Action::Quit
        ));
        assert!(matches!(
            handle_key(
                &mut app,
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                ratatui::layout::Rect::new(0, 0, 120, 24)
            ),
            Action::Quit
        ));
        assert!(matches!(
            handle_key(
                &mut app,
                key(KeyCode::Char('y')),
                ratatui::layout::Rect::new(0, 0, 120, 24)
            ),
            Action::Control(Operation::Shutdown)
        ));
        assert!(!app.confirm_shutdown);
        handle_key(
            &mut app,
            key(KeyCode::Char('Q')),
            ratatui::layout::Rect::new(0, 0, 120, 24),
        );
        handle_key(
            &mut app,
            key(KeyCode::Esc),
            ratatui::layout::Rect::new(0, 0, 120, 24),
        );
        assert!(!app.confirm_shutdown && app.pending.is_none());
    }
    #[test]
    fn search_input_is_modal_unicode_bounded_and_escape_cancels_scanning() {
        let mut app = connected();
        app.focus(Focus::Logs);
        handle_key(&mut app, key(KeyCode::Char('/')), area());
        for c in ['q', '界', 's'] {
            assert!(matches!(
                handle_key(&mut app, key(KeyCode::Char(c)), area()),
                Action::Redraw
            ));
        }
        assert_eq!(app.search_input.as_deref(), Some("q界s"));
        assert!(app.pending.is_none());
        handle_key(&mut app, key(KeyCode::Backspace), area());
        handle_key(&mut app, key(KeyCode::Enter), area());
        assert_eq!(app.logs.search.as_ref().unwrap().text, "q界");
        assert!(app.logs.paused);
        handle_key(&mut app, key(KeyCode::Esc), area());
        assert!(app.logs.search.is_none());
        assert_eq!(app.focus, Focus::Logs);
        handle_key(&mut app, key(KeyCode::Char('/')), area());
        app.search_input = Some("界".repeat(85));
        handle_key(&mut app, key(KeyCode::Char('界')), area());
        assert_eq!(app.search_input.as_ref().unwrap().len(), 255);
        handle_key(&mut app, key(KeyCode::Esc), area());
        assert!(app.search_input.is_none());
    }
    #[test]
    fn replacing_a_search_at_the_same_cursor_invalidates_old_requests_and_respects_modals() {
        let mut app = connected();
        app.focus(Focus::Logs);
        app.logs.start_search("first".into(), false, true, 10);
        let previous = app.logs.requested;
        assert!(matches!(
            handle_key(&mut app, key(KeyCode::Char('/')), area()),
            Action::Search
        ));
        app.search_input = Some("second".into());
        assert!(matches!(
            handle_key(&mut app, key(KeyCode::Enter), area()),
            Action::Search
        ));
        assert_eq!(app.logs.requested, previous);
        assert_eq!(app.logs.search.as_ref().unwrap().text, "second");
        app.help = true;
        handle_key(&mut app, key(KeyCode::Esc), area());
        assert!(!app.help && app.logs.search.is_some());
        app.logs.begin_selection(10);
        handle_key(&mut app, key(KeyCode::Esc), area());
        assert!(app.logs.selection.is_none() && app.logs.search.is_some());
        assert!(matches!(
            handle_key(&mut app, key(KeyCode::Esc), area()),
            Action::Search
        ));
        assert!(app.logs.search.is_none() && app.focus == Focus::Logs);
        handle_key(&mut app, key(KeyCode::Esc), area());
        assert_eq!(app.focus, Focus::Projects);
    }
}
