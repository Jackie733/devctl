use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph, Row, Table, Wrap},
};

use super::app::App;
use crate::{logs::Source, manager::ServiceState};

// Also sanitize names and diagnostics: only log text was sanitized by the backend.
fn clean(text: &str) -> String {
    let mut result = String::new();
    for character in text.chars() {
        if character == '\t' {
            result.push_str("    ");
        } else if character.is_control() {
            result.extend(character.escape_default());
        } else {
            result.push(character);
        }
    }
    result
}

pub fn log_height(height: u16) -> usize {
    usize::from(height.saturating_sub(9)).max(1)
}

fn state(app: &App, id: &str) -> (&'static str, Color) {
    if !app.connected {
        return ("Unknown", Color::Yellow);
    }
    match app.services[id].state {
        ServiceState::Stopped => ("Stopped", Color::DarkGray),
        ServiceState::Starting => ("Starting", Color::Yellow),
        ServiceState::Running => ("Running", Color::Green),
        ServiceState::Stopping => ("Stopping", Color::Yellow),
        ServiceState::Failed => ("Failed", Color::Red),
    }
}

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let connection = if app.connected {
        "Connected"
    } else {
        "Disconnected / state unknown"
    };
    let selected = app.selected.as_deref().unwrap_or("(none)");
    let selected_state = app
        .selected
        .as_ref()
        .filter(|id| app.services.contains_key(*id))
        .map_or("Unknown", |id| state(app, id).0);
    if area.width < 30 || area.height < 9 {
        let name: String = clean(selected).chars().take(12).collect();
        frame.render_widget(Paragraph::new(format!(
            "devctl\n{connection}\n{name}: {selected_state}\nUp/Down: select\nq: leave | Q: end\n?: help"
        )), area);
        overlay(frame, app);
        return;
    }

    let [header, summary, body, details, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(3),
        Constraint::Length(2),
    ])
    .areas(area);
    let identity: String = app
        .workspace_id
        .as_deref()
        .unwrap_or("-")
        .chars()
        .take(8)
        .collect();
    frame.render_widget(
        Paragraph::new(format!("devctl | {connection} | {}", clean(&identity))),
        header,
    );
    let pending = app
        .pending
        .as_ref()
        .map(|op| format!(" | Pending: {}", clean(&op.label())))
        .unwrap_or_default();
    frame.render_widget(
        Paragraph::new(format!(
            "Selected: {} | {selected_state}{pending}",
            clean(selected)
        )),
        summary,
    );

    let logs_area = if area.width >= 72 {
        let [projects, logs] =
            Layout::horizontal([Constraint::Length(30), Constraint::Min(0)]).areas(body);
        projects_view(frame, projects, app);
        logs
    } else {
        body
    };
    logs_view(frame, logs_area, app);

    let failure = app
        .selected
        .as_ref()
        .and_then(|id| app.services.get(id))
        .and_then(|status| {
            status.error.as_deref().map(|error| {
                format!(
                    "{}{}",
                    if status.cleanup_confirmed {
                        ""
                    } else {
                        "Cleanup UNCONFIRMED: "
                    },
                    clean(error)
                )
            })
        })
        .unwrap_or_default();
    let warnings = format!(
        "Logs: backend lost/evicted={}{}{} | local evicted={}{}",
        app.logs.dropped,
        if app.logs.gap { " GAP" } else { "" },
        if app.logs.incomplete {
            " CAPTURE INCOMPLETE"
        } else {
            ""
        },
        app.logs.local_evicted,
        if app.logs.anchor_lost {
            " READ POSITION EVICTED"
        } else {
            ""
        }
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(
                app.connection_error
                    .as_deref()
                    .map(clean)
                    .unwrap_or(failure),
                Style::default().fg(Color::Red),
            ),
            Line::from(clean(&app.feedback)),
            Line::styled(warnings, Style::default().fg(Color::Yellow)),
        ]),
        details,
    );
    let keys = if area.width >= 100 {
        "Up/Down select | s start | x stop | a start all | X stop all | q leave\nPgUp/PgDn scroll | Home oldest | End follow | Left/Right pan | r reconnect | Q end | ? help"
    } else {
        "s/x start/stop | q leave\na/X all | Q end | ? help"
    };
    frame.render_widget(Paragraph::new(keys), footer);
    overlay(frame, app);
}

fn projects_view(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::bordered().title("Projects");
    let height = usize::from(block.inner(area).height);
    let selected = app
        .services
        .keys()
        .position(|id| Some(id) == app.selected.as_ref())
        .unwrap_or(0);
    let start = selected.saturating_sub(height.saturating_sub(1));
    let rows = app.services.keys().skip(start).take(height).map(|id| {
        let (status, color) = state(app, id);
        let selected = Some(id) == app.selected.as_ref();
        let row = Row::new(vec![
            format!("{}{}", if selected { "> " } else { "  " }, clean(id)),
            status.into(),
        ]);
        row.style(if selected {
            Style::default().fg(Color::White).bg(Color::DarkGray)
        } else {
            Style::default().fg(color)
        })
    });
    frame.render_widget(
        Table::new(rows, [Constraint::Min(0), Constraint::Length(8)]).block(block),
        area,
    );
}

fn logs_view(frame: &mut Frame, area: Rect, app: &App) {
    let title = format!(
        "Logs: {} | {}",
        clean(app.selected.as_deref().unwrap_or("(none)")),
        if app.logs.top.is_none() {
            "following"
        } else {
            "reading"
        }
    );
    let block = Block::bordered().title(title);
    let height = usize::from(block.inner(area).height);
    if app.logs.records.is_empty() {
        frame.render_widget(
            Paragraph::new(if app.connected {
                "No recent output."
            } else {
                "Disconnected; reconnecting to the existing workspace."
            })
            .block(block),
            area,
        );
        return;
    }
    let lines: Vec<_> = app
        .logs
        .records
        .iter()
        .skip(app.logs.start(height))
        .take(height)
        .map(|record| {
            let (source, color) = match record.source {
                Source::Stdout => ("stdout", Color::Reset),
                Source::Stderr => ("stderr", Color::LightRed),
            };
            Line::from(vec![
                Span::styled(
                    format!("[{source} #{} seq={}] ", record.instance, record.sequence),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(clean(&record.text), Style::default().fg(color)),
                Span::styled(
                    if record.truncated { " [truncated]" } else { "" },
                    Style::default().fg(Color::Yellow),
                ),
            ])
        })
        .collect();
    frame.render_widget(
        Paragraph::new(lines)
            .scroll((0, app.logs.horizontal))
            .block(block),
        area,
    );
}

fn overlay(frame: &mut Frame, app: &App) {
    let (title, text) = if app.confirm_shutdown {
        (
            "End workspace",
            "Stop ALL services and end the background workspace?\n\ny: confirm | n / Esc: cancel\nq / Ctrl-C: leave the interface only",
        )
    } else if app.help {
        (
            "Help",
            "Up / Down: select a project\ns / x: start / stop selected project\na: start default services | X: stop ALL services\nPageUp / PageDown: scroll recent logs\nHome: oldest cached output | End: follow output\nLeft / Right: pan long log lines\nr: reconnect to the existing workspace\nQ: confirm stopping all services and ending workspace\nq / Ctrl-C: leave; services keep running\n\nRunning means process alive, not business readiness.\nLogs are bounded; gaps and losses are reported.\n\n? / Esc: close help",
        )
    } else {
        return;
    };
    let area = frame.area();
    let width = area.width.min(68);
    let height = area.height.min(if app.confirm_shutdown { 8 } else { 17 });
    let popup = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .block(Block::bordered().title(title)),
        popup,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{control::Response, manager::ServiceStatus};
    use ratatui::{Terminal, backend::TestBackend};

    fn draw(app: &App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| render(frame, app)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn wide_and_narrow_views_show_real_status_and_mark_disconnected_cache_unknown() {
        let mut app = App::new();
        app.apply_status(Response {
            workspace_id: "one".into(),
            services: [(
                "web".into(),
                ServiceStatus {
                    state: ServiceState::Failed,
                    error: Some("exit code 7".into()),
                    cleanup_confirmed: false,
                    ..ServiceStatus::default()
                },
            )]
            .into(),
            ..Response::default()
        });
        let wide = draw(&app, 120, 24);
        assert!(
            wide.contains("Projects") && wide.contains("Failed") && wide.contains("exit code 7")
        );
        assert!(wide.contains("Cleanup UNCONFIRMED"));
        let narrow = draw(&app, 50, 14);
        assert!(narrow.contains("Selected: web | Failed"));
        assert!(narrow.contains("q leave"));
        app.disconnect("connection lost".into());
        let disconnected = draw(&app, 120, 24);
        assert!(disconnected.contains("Unknown") && disconnected.contains("state unknown"));
        assert!(!disconnected.contains("Selected: web | Failed"));
    }

    #[test]
    fn small_windows_and_dialogs_render_without_panicking() {
        let mut app = App::new();
        for (width, height) in [(0, 0), (1, 1), (20, 6), (80, 24)] {
            draw(&app, width, height);
            app.help = true;
            draw(&app, width, height);
            app.confirm_shutdown = true;
            draw(&app, width, height);
            app.help = false;
            app.confirm_shutdown = false;
        }
        app.confirm_shutdown = true;
        assert!(draw(&app, 100, 24).contains("y: confirm"));
    }

    #[test]
    fn external_names_and_diagnostics_cannot_emit_terminal_controls() {
        assert_eq!(clean("name\x1b[2J\r\n\t"), "name\\u{1b}[2J\\r\\n    ");
    }
}
