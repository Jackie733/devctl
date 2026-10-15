use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph, Row, Table},
};

use super::app::{App, Focus};
use crate::{logs::Source, manager::ServiceState};

// Diagnostics and service IDs are external text too, not just log records.
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

pub struct ScreenLayout {
    pub usable: bool,
    pub wide: bool,
    pub header: Rect,
    pub projects: Option<Rect>,
    pub logs: Rect,
    pub details: Rect,
    pub footer: Rect,
}

pub fn usable(area: Rect) -> bool {
    area.width >= 36 && area.height >= 10
}

fn diagnostics(app: &App) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut errors = Vec::new();
    if let Some(error) = &app.connection_error {
        errors.push(clean(error));
    }
    if let Some(id) = &app.selected
        && let Some(status) = app.services.get(id)
        && let Some(error) = &status.error
    {
        errors.push(format!(
            "{}: {}{}",
            clean(id),
            clean(error),
            if status.cleanup_confirmed {
                " | cleanup confirmed"
            } else {
                ""
            }
        ));
    }
    let cleanup_unknown = app
        .selected
        .as_ref()
        .and_then(|id| app.services.get(id))
        .is_some_and(|status| status.state == ServiceState::Failed && !status.cleanup_confirmed);
    if !errors.is_empty() || cleanup_unknown {
        let priority = if cleanup_unknown {
            "Cleanup UNCONFIRMED | "
        } else {
            ""
        };
        lines.push(Line::styled(
            format!("{priority}{}", errors.join(" | ")),
            Style::default().fg(Color::LightRed),
        ));
    }
    lines.push(Line::from(clean(&app.feedback)));
    let mut warnings = Vec::new();
    if app.logs.dropped > 0 {
        warnings.push(format!("backend lost/evicted={}", app.logs.dropped));
    }
    if app.logs.local_evicted > 0 {
        warnings.push(format!("local evicted={}", app.logs.local_evicted));
    }
    if app.logs.gap {
        warnings.push("GAP".into());
    }
    if app.logs.incomplete {
        warnings.push("CAPTURE INCOMPLETE".into());
    }
    if app.logs.anchor_lost {
        warnings.push("READ POSITION EVICTED".into());
    }
    if let Some(error) = &app.logs_error {
        warnings.push(format!("Logs: {}", clean(error)));
    }
    if !warnings.is_empty() {
        lines.push(Line::styled(
            warnings.join(" | "),
            Style::default().fg(Color::Yellow),
        ));
    }
    lines
}

pub fn layout(area: Rect, app: &App) -> ScreenLayout {
    let wide = area.width >= 80 && area.height >= 12;
    let [header, body, details, footer] = Layout::vertical([
        Constraint::Length(if wide { 1 } else { 2 }),
        Constraint::Min(0),
        Constraint::Length(diagnostics(app).len() as u16),
        Constraint::Length(2),
    ])
    .areas(area);
    let (projects, logs) = if wide {
        let [projects, logs] = Layout::horizontal([
            Constraint::Length(((u32::from(area.width) * 3 / 10) as u16).clamp(24, 32)),
            Constraint::Min(0),
        ])
        .areas(body);
        (Some(projects), logs)
    } else {
        // The hidden log pane uses the same content height when paging from Projects.
        ((app.focus == Focus::Projects).then_some(body), body)
    };
    ScreenLayout {
        usable: usable(area),
        wide,
        header,
        projects,
        logs,
        details,
        footer,
    }
}

pub fn log_height(area: Rect, app: &App) -> usize {
    usize::from(layout(area, app).logs.height.saturating_sub(2)).max(1)
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
        ServiceState::Failed => ("Failed", Color::LightRed),
    }
}

fn selected_summary(app: &App) -> String {
    let id = app.selected.as_deref().unwrap_or("(none)");
    let status = if app.services.contains_key(id) {
        state(app, id).0
    } else {
        "Unknown"
    };
    format!("Selected: {} | {status}", clean(id))
}

fn panel(title: String, active: bool) -> Block<'static> {
    Block::bordered()
        .title(title)
        .border_style(Style::default().fg(if active { Color::Cyan } else { Color::DarkGray }))
}

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let connection = if app.connected {
        "Connected"
    } else if app.workspace_id.is_none() && app.connection_error.is_none() {
        "Connecting"
    } else {
        "Disconnected / state unknown"
    };
    let screen = layout(area, app);
    if !screen.usable {
        frame.render_widget(Paragraph::new(format!(
            "q: leave | Ctrl-C: leave\nWindow too small (min 36x10).\n{connection}\nEnlarge to control services."
        )), area);
        return;
    }
    let identity: String = clean(app.workspace_id.as_deref().unwrap_or("-"))
        .chars()
        .take(8)
        .collect();
    let mut header = vec![Line::from(format!(
        "devctl | {connection} | workspace {identity}"
    ))];
    if screen.header.height > 1 {
        header.push(Line::from(format!("{} | 1/0 panel", selected_summary(app))));
    }
    frame.render_widget(Paragraph::new(header), screen.header);
    if let Some(projects) = screen.projects {
        projects_view(frame, projects, app, screen.wide);
    }
    if screen.wide || app.focus == Focus::Logs {
        logs_view(frame, screen.logs, app);
    }
    frame.render_widget(Paragraph::new(diagnostics(app)), screen.details);
    let id = clean(app.selected.as_deref().unwrap_or("(none)"));
    let context = if app.goto_top {
        "g ... | g: top | Esc: cancel".into()
    } else if app.focus == Focus::Projects && area.width < 80 {
        format!("s/x {id} | j/k select | Enter logs")
    } else if app.focus == Focus::Projects {
        format!("j/k select | Enter logs | s start {id} | x stop {id} | Tab panel")
    } else if area.width >= 80 {
        "j/k scroll | gg oldest | G follow | Ctrl-u/d half page | H/L pan | Esc projects".into()
    } else {
        "j/k scroll | G follow | Esc projects".into()
    };
    // Put escape and help first so they survive even the smallest usable width.
    let global = if area.width >= 100 {
        "q leave (keep running) | ? help | a start default | X stop ALL | Q end | r sync"
    } else if area.width >= 60 {
        "q leave | ? help | a default | X stop ALL | Q end | r sync"
    } else {
        "q leave | ? help | Q end | a default"
    };
    frame.render_widget(
        Paragraph::new(vec![Line::from(context), Line::from(global)]),
        screen.footer,
    );
    overlay(frame, app);
}

fn projects_view(frame: &mut Frame, area: Rect, app: &App, show_details: bool) {
    let active = app.focus == Focus::Projects;
    let block = panel(
        format!("[1] Projects{}", if active { " · ACTIVE" } else { "" }),
        active,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    // Compact mode already shows the selection in the header; give its rows to projects.
    let detail_height = if show_details {
        inner.height.saturating_sub(1).min(2)
    } else {
        0
    };
    let [list, summary] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(detail_height)]).areas(inner);
    let height = usize::from(list.height);
    let selected = app
        .services
        .keys()
        .position(|id| Some(id) == app.selected.as_ref())
        .unwrap_or(0);
    let start = selected.saturating_sub(height.saturating_sub(1));
    let rows = app.services.keys().skip(start).take(height).map(|id| {
        let (status, color) = state(app, id);
        let selected = Some(id) == app.selected.as_ref();
        Row::new(vec![
            format!("{}{}", if selected { "> " } else { "  " }, clean(id)),
            status.into(),
        ])
        .style(if selected && active {
            Style::default()
                .fg(Color::White)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(color)
        })
    });
    frame.render_widget(
        Table::new(rows, [Constraint::Min(0), Constraint::Length(8)]),
        list,
    );
    let detail = app
        .selected
        .as_ref()
        .and_then(|id| app.services.get(id))
        .and_then(|status| status.error.as_deref())
        .map(clean)
        .unwrap_or_else(|| {
            if app.services.is_empty() {
                "No projects available.".into()
            } else {
                "1 Projects / 0 Logs | ? details".into()
            }
        });
    let selected = selected_summary(app);
    let selected = if Line::from(selected.as_str()).width() > usize::from(summary.width) {
        selected
            .strip_prefix("Selected: ")
            .unwrap_or(&selected)
            .to_owned()
    } else {
        selected
    };
    frame.render_widget(
        Paragraph::new(vec![Line::from(selected), Line::from(detail)]),
        summary,
    );
}

fn logs_view(frame: &mut Frame, area: Rect, app: &App) {
    let height = usize::from(area.height.saturating_sub(2)).max(1);
    let start = app.logs.start(height);
    let position = if app.logs.top.is_none() {
        "FOLLOW".into()
    } else {
        format!(
            "READ {}–{} / {} cached",
            start + 1,
            (start + height).min(app.logs.records.len()),
            app.logs.records.len()
        )
    };
    let offset = if app.logs.horizontal == 0 {
        String::new()
    } else {
        format!(" | col +{}", app.logs.horizontal)
    };
    let title = format!(
        "[0] Logs{}: {} | {position}{offset}",
        if app.focus == Focus::Logs {
            " · ACTIVE"
        } else {
            ""
        },
        clean(app.selected.as_deref().unwrap_or("(none)")),
    );
    let block = panel(title, app.focus == Focus::Logs);
    if app.logs.records.is_empty() {
        let empty = if !app.connected {
            "Disconnected; state unknown. Reconnecting to the existing workspace.".into()
        } else if app.selected.is_none() {
            "No project selected.".into()
        } else if let Some(error) = &app.logs_error {
            format!("Log read failed: {}", clean(error))
        } else if !app.logs_loaded {
            "Reading recent output...".into()
        } else if app
            .selected
            .as_ref()
            .is_some_and(|id| app.services[id].state == ServiceState::Running)
        {
            "Running; no output received yet.".into()
        } else {
            "No recent output; project is not running.".into()
        };
        frame.render_widget(Paragraph::new(empty).block(block), area);
        return;
    }
    let lines: Vec<_> = app
        .logs
        .records
        .iter()
        .skip(start)
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

fn popup(area: Rect, width: u16, height: u16) -> Rect {
    let width = area.width.min(width);
    let height = area.height.min(height);
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}

fn shutdown_text(app: &App) -> String {
    format!(
        "Stop ALL {} projects and end the background workspace?\n\ny: confirm | n / Esc: cancel\nq / Ctrl-C: leave the interface only",
        app.services.len()
    )
}

// Wrap by terminal columns using Ratatui's width calculation. Rendering and scroll
// bounds consume these same lines, including long IDs without word boundaries.
fn wrapped(lines: Vec<Line<'static>>, width: u16) -> Vec<Line<'static>> {
    if width == 0 {
        return Vec::new();
    }
    let mut result = Vec::new();
    for line in lines {
        let mut row = String::new();
        let mut columns = 0;
        for grapheme in line.styled_graphemes(Style::default()) {
            let glyph = grapheme.symbol;
            let size = Span::raw(glyph).width();
            if columns + size > usize::from(width) && !row.is_empty() {
                result.push(Line::styled(std::mem::take(&mut row), line.style));
                columns = 0;
            }
            row.push_str(glyph);
            columns += size;
        }
        result.push(Line::styled(row, line.style));
    }
    result
}

fn shutdown_lines(app: &App, width: u16) -> Vec<Line<'static>> {
    wrapped(
        shutdown_text(app)
            .lines()
            .map(|line| Line::from(line.to_owned()))
            .collect(),
        width,
    )
}

pub fn can_confirm(area: Rect, app: &App) -> bool {
    let popup = popup(area, 68, 9);
    usable(area)
        && shutdown_lines(app, popup.width.saturating_sub(2)).len()
            <= usize::from(popup.height.saturating_sub(2))
}

fn help_lines(app: &App) -> Vec<Line<'static>> {
    let context = match app.focus {
        Focus::Projects => "j/k or Up/Down: select | gg/G: first/last | Enter: logs",
        Focus::Logs => "j/k or Up/Down: scroll | gg: oldest | G: follow | Esc: projects",
    };
    let mut lines: Vec<Line<'static>> = vec![
        "CURRENT PANEL",
        context,
        "h/l, Tab/Shift-Tab: focus panel | 1 Projects / 0 Logs",
        "",
        "SELECTED PROJECT",
        "s: start selected | x: stop selected",
        "",
        "WORKSPACE",
        "a: start default (autostart=true) | X: stop ALL; keep background",
        "Q: confirm ending workspace | r: sync existing background",
        "",
        "LOGS",
        "Ctrl-u/d: half page | PageUp/PageDown: page",
        "Home: oldest cached | End: follow | H/L or Left/Right: pan",
        "Running means process alive, not business readiness.",
        "Logs are bounded; gaps and losses are reported.",
        "",
        "LEAVING",
        "q / Ctrl-C: leave; services keep running",
        "",
        "CURRENT DETAILS",
    ]
    .into_iter()
    .map(Line::from)
    .collect();
    lines.push(Line::from(selected_summary(app)));
    lines.extend(diagnostics(app));
    lines
}

pub fn help_max_scroll(area: Rect, app: &App) -> u16 {
    let area = popup(area, 90, area.height.saturating_sub(2));
    let height = usize::from(area.height.saturating_sub(3));
    let count = wrapped(help_lines(app), area.width.saturating_sub(2)).len();
    count.saturating_sub(height).min(usize::from(u16::MAX)) as u16
}

fn overlay(frame: &mut Frame, app: &App) {
    if app.confirm_shutdown {
        let area = popup(frame.area(), 68, 9);
        frame.render_widget(Clear, area);
        let text = if can_confirm(frame.area(), app) {
            shutdown_lines(app, area.width.saturating_sub(2))
        } else {
            vec![
                Line::from("Enlarge window to confirm."),
                Line::from("n / Esc: cancel | q: leave"),
            ]
        };
        frame.render_widget(
            Paragraph::new(text).block(Block::bordered().title("End workspace")),
            area,
        );
    } else if app.help {
        let area = popup(frame.area(), 90, frame.area().height.saturating_sub(2));
        frame.render_widget(Clear, area);
        let block = Block::bordered().title("Help | j/k scroll | gg/G first/last");
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let [body, footer] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);
        frame.render_widget(
            Paragraph::new(wrapped(help_lines(app), body.width))
                .scroll((app.help_scroll.min(help_max_scroll(frame.area(), app)), 0)),
            body,
        );
        let prefix = if app.goto_top { "g ... | " } else { "" };
        frame.render_widget(
            Paragraph::new(format!("{prefix}?/Esc close | q leave | j/k scroll")),
            footer,
        );
    }
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
    fn responsive_layout_preserves_focus_and_reserves_the_real_log_height() {
        let mut app = App::new();
        app.apply_status(Response {
            workspace_id: "one".into(),
            services: [("web".into(), ServiceStatus::default())].into(),
            ..Response::default()
        });
        for (width, height) in [(120, 30), (80, 12), (60, 16), (36, 10)] {
            let area = Rect::new(0, 0, width, height);
            for focus in [Focus::Projects, Focus::Logs] {
                app.focus(focus);
                let screen = layout(area, &app);
                assert!(screen.usable);
                assert_eq!(log_height(area, &app), usize::from(screen.logs.height - 2));
                assert!(log_height(area, &app) >= if width >= 80 && height >= 12 { 4 } else { 1 });
                let text = draw(&app, width, height);
                assert!(text.contains("q leave") && text.contains("? help"));
                assert!(text.contains(if focus == Focus::Projects {
                    "Projects · ACTIVE"
                } else {
                    "Logs · ACTIVE"
                }));
                assert_eq!(app.selected.as_deref(), Some("web"));
                assert_eq!(app.focus, focus);
            }
        }
    }

    #[test]
    fn empty_logs_distinguish_loading_failure_stopped_and_running() {
        let mut app = App::new();
        app.apply_status(Response {
            workspace_id: "one".into(),
            services: [("web".into(), ServiceStatus::default())].into(),
            ..Response::default()
        });
        assert!(draw(&app, 120, 30).contains("Reading recent output"));
        app.apply_logs(crate::logs::Batch::default());
        assert!(draw(&app, 120, 30).contains("project is not running"));
        app.services.get_mut("web").unwrap().state = ServiceState::Running;
        // Live services have not been cleaned up yet; this is not a cleanup failure.
        app.services.get_mut("web").unwrap().cleanup_confirmed = false;
        assert!(draw(&app, 120, 30).contains("no output received yet"));
        assert!(!draw(&app, 120, 30).contains("Cleanup UNCONFIRMED"));
        app.logs_error = Some("permission denied".into());
        assert!(draw(&app, 120, 30).contains("Log read failed: permission denied"));
        app.apply_logs(crate::logs::Batch::default());
        assert!(!draw(&app, 120, 30).contains("permission denied"));
        app.disconnect("lost".into());
        assert!(draw(&app, 120, 30).contains("state unknown"));
    }

    #[test]
    fn help_reaches_full_unicode_diagnostics_and_restores_its_footer() {
        let rows = wrapped(vec![Line::from("ab👩‍💻e\u{301}界")], 3);
        let text: Vec<_> = rows
            .iter()
            .map(|line| line.spans[0].content.as_ref())
            .collect();
        assert_eq!(text, ["ab", "👩‍💻e\u{301}", "界"]);
        let mut app = App::new();
        let id = "项目名称很长".repeat(10);
        let error = format!("{} END-OF-DIAGNOSTIC", "错误原因".repeat(80));
        app.apply_status(Response {
            workspace_id: "one".into(),
            services: [(
                id.clone(),
                ServiceStatus {
                    error: Some(error),
                    ..ServiceStatus::default()
                },
            )]
            .into(),
            ..Response::default()
        });
        app.help = true;
        let area = Rect::new(0, 0, 36, 10);
        let max = help_max_scroll(area, &app);
        assert!(max > 50);
        // Every character of a long ID must remain reachable through vertical scrolling.
        let wrapped = wrapped(help_lines(&app), 32);
        assert!(wrapped.iter().all(|line| line.width() <= 32));
        let flattened: String = wrapped
            .iter()
            .flat_map(|line| line.spans.iter().map(|span| span.content.as_ref()))
            .collect();
        assert!(flattened.contains(&id));
        for scroll in 0..=max {
            app.help_scroll = scroll;
            if draw(&app, 36, 10).contains("END-OF-DIAGNOSTIC") {
                return;
            }
        }
        panic!("The full diagnostic tail was unreachable");
    }

    #[test]
    fn diagnostics_do_not_hide_unknown_cleanup_and_confirm_requires_visible_text() {
        let mut app = App::new();
        app.apply_status(Response {
            workspace_id: "one".into(),
            services: [(
                "web".into(),
                ServiceStatus {
                    state: ServiceState::Failed,
                    error: Some("failed".into()),
                    cleanup_confirmed: false,
                    ..ServiceStatus::default()
                },
            )]
            .into(),
            ..Response::default()
        });
        app.feedback = "Start request accepted".into();
        app.logs.gap = true;
        let area = Rect::new(0, 0, 80, 12);
        assert!(draw(&app, 80, 12).contains("Cleanup UNCONFIRMED"));
        assert!(draw(&app, 80, 12).contains("GAP"));
        assert_eq!(log_height(area, &app), 4);
        assert!(can_confirm(area, &app));
        assert!(!can_confirm(Rect::new(0, 0, 20, 6), &app));
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
