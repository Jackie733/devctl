use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Padding, Paragraph, Row, Table},
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
    let mut warnings = Vec::new();
    if let Some(info) = &app.logs.storage {
        lines.push(Line::from(format!(
            "Saving: {} | session {} | {}",
            clean(&info.state),
            clean(&info.session),
            clean(&info.directory)
        )));
        if !info.warning.is_empty() {
            warnings.push(clean(&info.warning));
        }
    }
    if app.logs.dropped > 0 {
        warnings.push(format!("backend lost/evicted={}", app.logs.dropped));
    }
    if app.logs.local_evicted > 0 {
        warnings.push(format!("client cache trimmed={}", app.logs.local_evicted));
    }
    if app.logs.gap {
        warnings.push("GAP".into());
    }
    if app.logs.incomplete {
        warnings.push("OUTPUT INCOMPLETE".into());
    }
    if app.logs.selection_lost {
        warnings.push("SELECTION EVICTED: select again".into());
    }
    if app.logs.anchor_lost {
        warnings.push("READ POSITION EVICTED".into());
    }
    if let Some(error) = &app.logs_error {
        warnings.push(format!("Logs: {}", clean(error)));
    }
    if app.logs_loaded && !app.logs.records.is_empty() && app.logs.oldest_retained.is_none() {
        warnings.push("History paging unavailable on this background version; end the workspace and reopen with the updated program.".into());
    }
    if !warnings.is_empty() {
        lines.push(Line::styled(
            warnings.join(" | "),
            Style::default().fg(Color::Yellow),
        ));
    }
    lines.push(Line::from(clean(&app.feedback)));
    lines
}

fn status_line(app: &App) -> Line<'static> {
    let mut flags = Vec::new();
    let selected = app.selected.as_ref().and_then(|id| app.services.get(id));
    let cleanup_unknown = selected
        .is_some_and(|status| status.state == ServiceState::Failed && !status.cleanup_confirmed);
    if !app.connected && (app.workspace_id.is_some() || app.connection_error.is_some()) {
        flags.push("Disconnected / state unknown");
    }
    if cleanup_unknown {
        flags.push("Cleanup UNCONFIRMED");
    }
    if app
        .logs
        .storage
        .as_ref()
        .is_some_and(|s| s.state == "degraded")
    {
        flags.push("SAVE DEGRADED");
    }
    if app.logs.selection_lost {
        flags.push("SELECTION EVICTED");
    }
    if app.logs.gap {
        flags.push("GAP");
    }
    if app.logs.incomplete {
        flags.push("OUTPUT INCOMPLETE");
    }
    if app.logs.anchor_lost {
        flags.push("READ POSITION EVICTED");
    }
    if app.logs.dropped > 0 {
        flags.push("LOG LOSS");
    }
    if app.logs_error.is_some() {
        flags.push("LOG READ FAILED");
    }
    if app.connected && selected.is_some_and(|status| status.state == ServiceState::Failed) {
        flags.push("PROJECT FAILED");
    }
    let style = if cleanup_unknown || !app.connected && app.connection_error.is_some() {
        Style::default().fg(Color::LightRed)
    } else if !flags.is_empty() {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let mut text = flags.join(" | ");
    if !text.is_empty() {
        text.push_str(" | ");
    }
    if let Some(error) = selected.and_then(|status| status.error.as_deref()) {
        text.push_str(&clean(error));
        text.push_str(" | ");
    }
    if let Some(input) = &app.search_input {
        text.push_str(&format!("/{} | Enter search | Esc cancel", clean(input)));
    } else {
        text.push_str(&clean(&app.feedback));
    }
    Line::styled(text, style)
}

pub fn layout(area: Rect, app: &App) -> ScreenLayout {
    let wide = area.width >= 80 && area.height >= 12;
    let [header, body, details, footer] = Layout::vertical([
        Constraint::Length(if wide { 1 } else { 2 }),
        Constraint::Min(0),
        Constraint::Length(1),
        Constraint::Length(1),
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

fn log_areas(area: Rect) -> [Rect; 2] {
    Layout::vertical([Constraint::Length(1), Constraint::Min(0)])
        .areas(Block::bordered().inner(area))
}

pub fn log_height(area: Rect, app: &App) -> usize {
    usize::from(log_areas(layout(area, app).logs)[1].height).max(1)
}

fn log_notice(app: &App) -> (&'static str, &'static str, Color) {
    if !app.connected {
        return (
            "UNKNOWN",
            "Current state unverified; r sync.",
            Color::Yellow,
        );
    }
    let Some(status) = app.selected.as_ref().and_then(|id| app.services.get(id)) else {
        return ("NO PROJECT", "Choose a project.", Color::DarkGray);
    };
    match status.state {
        ServiceState::Stopped => ("STOPPED", "Retained logs; process stopped.", Color::Yellow),
        ServiceState::Starting => (
            "STARTING",
            "Waiting for process state; earlier logs may remain.",
            Color::Yellow,
        ),
        ServiceState::Running => (
            "RUNNING",
            "Process alive; earlier runs are marked old.",
            Color::Green,
        ),
        ServiceState::Stopping => (
            "STOPPING",
            "Cleanup in progress; logs may still arrive.",
            Color::Yellow,
        ),
        ServiceState::Failed if !status.cleanup_confirmed => (
            "FAILED",
            "Cleanup UNCONFIRMED; check ? details.",
            Color::LightRed,
        ),
        ServiceState::Failed => (
            "FAILED",
            "Process ended; retained logs. ? details.",
            Color::LightRed,
        ),
    }
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
        "devctl {} | {connection} | workspace {identity}",
        env!("CARGO_PKG_VERSION")
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
    // Full diagnostics stay in Help; the main view reserves exactly two bottom rows.
    frame.render_widget(Paragraph::new(status_line(app)), screen.details);
    let context = if app.goto_top {
        "g ... | g top | Esc cancel".into()
    } else if app.logs.selection.is_some() && area.width < 60 {
        "v j/k y copy Esc".into()
    } else if app.logs.selection.is_some() {
        "V-LINE | j/k extend | y copy | Esc cancel".into()
    } else if app.focus == Focus::Projects {
        let id = clean(app.selected.as_deref().unwrap_or("(none)"));
        if area.width < 60 {
            format!("s/x {id} j/k Enter")
        } else {
            format!("s/x {id} | j/k select | Enter logs | a default | X stop ALL | Q end")
        }
    } else if area.width < 60 {
        "j/k v p G Esc".into()
    } else {
        "/ search | n/N match | v select | p pause | G follow".into()
    };
    let [context_area, global] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(17)]).areas(screen.footer);
    frame.render_widget(Paragraph::new(context), context_area);
    frame.render_widget(Paragraph::new("? help | q leave"), global);
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
    let [notice_area, content] = log_areas(area);
    let height = usize::from(content.height).max(1);
    let start = app.logs.start(height);
    let position = if let Some(selection) = app.logs.selection {
        let count = app
            .logs
            .records
            .iter()
            .filter(|record| selection.contains(record.sequence))
            .count();
        format!("V-LINE {count} selected")
    } else if !app.logs.paused {
        "FOLLOW".into()
    } else if app.logs.records.is_empty() {
        "READ 0 cached".into()
    } else {
        format!(
            "READ {}–{} / {} cached",
            start + 1,
            (start + height).min(app.logs.records.len()),
            app.logs.records.len()
        )
    };
    let history = if app.logs.paused {
        let boundary = if app.logs.requested.is_some() {
            "loading history"
        } else if app.logs.records.front().is_some_and(|first| {
            app.logs
                .oldest_retained
                .is_some_and(|oldest| oldest < first.sequence)
        }) {
            "earlier available"
        } else if app.logs.records.front().is_some_and(|first| {
            app.logs
                .oldest_retained
                .is_some_and(|oldest| first.sequence < oldest)
        }) {
            "snapshot predates retained history"
        } else if app.logs.oldest_retained.is_some() {
            "retained start"
        } else {
            ""
        };
        format!(" | +{} new | {boundary}", app.logs.new_records())
    } else {
        String::new()
    };
    let offset = if app.logs.horizontal == 0 {
        String::new()
    } else {
        format!(" | col +{}", app.logs.horizontal)
    };
    let (status, notice, color) = log_notice(app);
    let instance = app
        .selected
        .as_ref()
        .and_then(|id| app.services.get(id))
        .and_then(|status| status.instance);
    let instance_label = instance
        .map(|instance| format!(" | #{instance}"))
        .unwrap_or_default();
    let title = format!(
        "[0] Logs{} · {status}: {}{instance_label}",
        if app.focus == Focus::Logs {
            " · ACTIVE"
        } else {
            ""
        },
        clean(app.selected.as_deref().unwrap_or("(none)")),
    );
    let block =
        panel(title, app.focus == Focus::Logs).title_bottom(format!("{position}{history}{offset}"));
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(Line::styled(
            format!(
                "{status} | {notice} | save={}",
                app.logs
                    .storage
                    .as_ref()
                    .map_or("unknown", |s| s.state.as_str())
            ),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )),
        notice_area,
    );
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
        frame.render_widget(Paragraph::new(empty), content);
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
            let selected = app
                .logs
                .selection
                .is_some_and(|selection| selection.contains(record.sequence));
            let head = app
                .logs
                .selection
                .map(|selection| selection.head)
                .or(app.logs.reading)
                .is_some_and(|sequence| sequence == record.sequence);
            let row_style = if selected {
                Style::default()
                    .bg(if head { Color::Blue } else { Color::DarkGray })
                    .fg(Color::White)
            } else if head && app.focus == Focus::Logs {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };
            let history =
                if app.connected && instance.is_some_and(|current| record.instance < current) {
                    " old"
                } else {
                    ""
                };
            let pointer = if app.logs.selection.is_some() || app.logs.reading.is_some() {
                if head { "> " } else { "  " }
            } else {
                ""
            };
            let timestamp = if area.width >= 100 && record.captured_ms != 0 {
                let seconds = record.captured_ms / 1000;
                format!(
                    "UTC {:02}:{:02}:{:02} ",
                    (seconds / 3600) % 24,
                    (seconds / 60) % 60,
                    seconds % 60
                )
            } else {
                String::new()
            };
            Line::from(vec![
                Span::styled(timestamp, Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!(
                        "{pointer}[{source} #{}{history} seq={}] ",
                        record.instance, record.sequence
                    ),
                    if selected {
                        row_style
                    } else if head && app.focus == Focus::Logs {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ),
                Span::styled(
                    clean(&record.text),
                    if selected {
                        row_style
                    } else {
                        Style::default().fg(color)
                    },
                ),
                Span::styled(
                    if record.truncated { " [truncated]" } else { "" },
                    Style::default().fg(Color::Yellow),
                ),
            ])
            .style(row_style)
        })
        .collect();
    frame.render_widget(
        Paragraph::new(lines).scroll((0, app.logs.horizontal)),
        content,
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
    let context = if app.logs.selection.is_some() {
        "V-LINE: j/k extend | y copy | Esc cancel | gg/G first/last"
    } else {
        match app.focus {
            Focus::Projects => "j/k or Up/Down: select | gg/G: first/last | Enter: logs",
            Focus::Logs => {
                "j/k or Up/Down: move log cursor | gg: oldest | G: follow | Esc: projects"
            }
        }
    };
    let panels = if app.logs.selection.is_some() {
        "Cancel selection (Esc) before panel or service controls."
    } else {
        "h/l, Tab/Shift-Tab: focus panel | 1 Projects / 0 Logs"
    };
    let mut lines: Vec<Line<'static>> = vec![
        "CURRENT PANEL",
        context,
        panels,
        "SELECTED PROJECT",
        "s: start selected | x: stop selected",
        "",
        "WORKSPACE",
        "a: start default (autostart=true) | X: stop ALL; keep background",
        "Q: confirm ending workspace | r: sync existing background",
        "",
        "LOGS",
        "Running means process alive, not business readiness.",
        "Ctrl-u/d: half page | PageUp/PageDown: page; edges load history",
        "Home/gg: oldest retained | End/G: follow | p: pause/resume",
        "j/k: move log cursor; v: select from cursor; j/k: extend selection",
        "/: literal case-sensitive search; Enter runs; Esc cancels; n/N next/previous",
        "Search fixes latest seq; / refreshes. Match opens context for v/y copy.",
        "In V-LINE: y copies log text; Esc cancels and stays in logs.",
        "Local WSL: copy via clip.exe; elsewhere: terminal clipboard (OSC 52).",
        "Copy feedback names the method; verify by pasting. Failures keep selection.",
        "Selected logs stay anchored; eviction cancels selection.",
        "Log header/banner shows service state; FOLLOW/READ is only scrolling.",
        "STOPPED/FAILED logs are retained history; old marks an earlier run.",
        "H/L or Left/Right: pan | Bounded logs; gaps/losses are reported.",
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

fn help_block() -> Block<'static> {
    Block::bordered()
        .title("Help | j/k scroll | gg/G first/last")
        .padding(Padding::new(2, 2, 1, 1))
}

fn help_area(area: Rect, app: &App) -> Rect {
    let width = area.width.saturating_sub(4).min(80);
    let inner = help_block().inner(Rect::new(0, 0, width, area.height));
    let rows = wrapped(help_lines(app), inner.width).len();
    // Borders, vertical padding and the fixed footer occupy five rows.
    let height = rows.saturating_add(5).min(usize::from(u16::MAX)) as u16;
    popup(area, width, height.min(area.height.saturating_sub(2)))
}

pub fn help_height(area: Rect, app: &App) -> u16 {
    help_block()
        .inner(help_area(area, app))
        .height
        .saturating_sub(1)
        .max(1)
}

pub fn help_max_scroll(area: Rect, app: &App) -> u16 {
    let inner = help_block().inner(help_area(area, app));
    let height = usize::from(inner.height.saturating_sub(1));
    let count = wrapped(help_lines(app), inner.width).len();
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
        let area = help_area(frame.area(), app);
        frame.render_widget(Clear, area);
        let block = help_block();
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
    fn log_state_uses_status_instead_of_ready_text_and_survives_scrolling() {
        let mut app = App::new();
        app.apply_status(Response {
            workspace_id: "one".into(),
            services: [(
                "web".into(),
                ServiceStatus {
                    instance: Some(5),
                    ..ServiceStatus::default()
                },
            )]
            .into(),
            ..Response::default()
        });
        app.apply_logs(crate::logs::Batch {
            records: [3, 5, 6]
                .into_iter()
                .enumerate()
                .map(|(index, instance)| crate::logs::Record {
                    captured_ms: 0,
                    sequence: index as u64 + 1,
                    instance,
                    source: Source::Stdout,
                    text: "VITE ready at http://localhost:3050".into(),
                    truncated: false,
                })
                .collect(),
            ..crate::logs::Batch::default()
        });
        let stopped = draw(&app, 120, 30);
        assert!(stopped.contains("Logs · STOPPED: web | #5"));
        assert!(stopped.contains("STOPPED | Retained logs"));
        assert!(stopped.contains("#3 old seq=1"));
        assert!(!stopped.contains("#5 old") && !stopped.contains("#6 old"));
        assert!(stopped.contains("VITE ready") && stopped.contains("FOLLOW"));
        for (status, label, message) in [
            (
                ServiceState::Starting,
                "STARTING",
                "Waiting for process state",
            ),
            (ServiceState::Running, "RUNNING", "Process alive"),
            (ServiceState::Stopping, "STOPPING", "Cleanup in progress"),
            (ServiceState::Failed, "FAILED", "Process ended"),
        ] {
            app.services.get_mut("web").unwrap().state = status;
            let text = draw(&app, 120, 30);
            assert!(text.contains(&format!("Logs · {label}: web")) && text.contains(message));
            assert!(text.contains("VITE ready"));
        }
        app.services.get_mut("web").unwrap().cleanup_confirmed = false;
        assert!(draw(&app, 120, 30).contains("FAILED | Cleanup UNCONFIRMED"));
        // The notice belongs to the view, not the selectable log records.
        app.focus(Focus::Logs);
        app.logs.begin_selection(20);
        app.logs.move_selection(1, 20);
        assert_eq!(
            app.logs.selected_text().unwrap().0,
            "VITE ready at http://localhost:3050\nVITE ready at http://localhost:3050\n"
        );
        app.logs.horizontal = 20;
        assert!(draw(&app, 36, 10).contains("FAILED | Cleanup UNCONFIRMED"));
        app.disconnect("lost".into());
        assert!(draw(&app, 120, 30).contains("Logs · ACTIVE · UNKNOWN"));
        assert!(!draw(&app, 120, 30).contains("Process ended"));
    }

    #[test]
    fn notice_is_fixed_while_text_scrolls_and_does_not_use_log_rows() {
        let mut app = App::new();
        app.apply_status(Response {
            workspace_id: "one".into(),
            services: [("web".into(), ServiceStatus::default())].into(),
            ..Response::default()
        });
        app.apply_logs(crate::logs::Batch {
            records: (1..=100)
                .map(|sequence| crate::logs::Record {
                    captured_ms: 0,
                    sequence,
                    instance: 1,
                    source: Source::Stdout,
                    text: format!("log {sequence}"),
                    truncated: false,
                })
                .collect(),
            ..crate::logs::Batch::default()
        });
        for (width, height) in [(120, 30), (80, 12), (36, 10)] {
            app.focus(Focus::Logs);
            let area = Rect::new(0, 0, width, height);
            let page = log_height(area, &app);
            let log_area = layout(area, &app).logs;
            let [notice, content] = log_areas(log_area);
            assert_eq!(usize::from(content.height), page);
            assert_eq!(notice.height, 1);
            app.logs.follow();
            app.logs.scroll(-(page as isize), page);
            let text = draw(&app, width, height);
            assert!(text.contains("STOPPED | Retained logs"));
            assert!(text.contains("READ"));
            app.logs.begin_selection(page);
            assert_eq!(app.logs.selected_text().unwrap().1, 1);
        }
    }

    #[test]
    fn bottom_uses_two_rows_even_with_errors_and_selection_is_visible() {
        let mut app = App::new();
        app.apply_status(Response {
            workspace_id: "one".into(),
            services: [("web".into(), ServiceStatus::default())].into(),
            ..Response::default()
        });
        app.apply_logs(crate::logs::Batch {
            records: (1..=3)
                .map(|sequence| crate::logs::Record {
                    captured_ms: 0,
                    sequence,
                    instance: 1,
                    source: Source::Stdout,
                    text: format!("body {sequence}"),
                    truncated: false,
                })
                .collect(),
            gap: true,
            ..crate::logs::Batch::default()
        });
        app.connection_error = Some("connection issue".into());
        for (width, height) in [(120, 30), (80, 12), (36, 10)] {
            let screen = layout(Rect::new(0, 0, width, height), &app);
            assert_eq!(screen.details.height + screen.footer.height, 2);
        }
        app.connection_error = None;
        app.focus(Focus::Logs);
        app.logs.begin_selection(20);
        app.logs.move_selection(1, 20);
        let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let log_area = layout(Rect::new(0, 0, 120, 30), &app).logs;
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(log_area.x + 1, log_area.y + 2)].bg, Color::DarkGray);
        assert_eq!(buffer[(log_area.x + 1, log_area.y + 3)].bg, Color::Blue);
        assert_ne!(buffer[(log_area.x + 1, log_area.y + 4)].bg, Color::Blue);
        let narrow = draw(&app, 36, 10);
        assert!(narrow.contains("V-LINE") && narrow.contains("y copy") && narrow.contains("Esc"));
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
                assert_eq!(log_height(area, &app), usize::from(screen.logs.height - 3));
                assert!(log_height(area, &app) >= if width >= 80 && height >= 12 { 4 } else { 1 });
                let text = draw(&app, width, height);
                assert!(text.starts_with(concat!(
                    "devctl ",
                    env!("CARGO_PKG_VERSION"),
                    " | Connected"
                )));
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
    fn help_fits_content_on_large_screens_and_keeps_details_reachable_after_resize() {
        let mut app = App::new();
        app.help = true;
        app.feedback = "HELP-TAIL".into();
        let large = Rect::new(7, 3, 300, 64);
        let popup = help_area(large, &app);
        assert_eq!(popup.width, 80);
        assert!(popup.height < 40);
        assert_eq!(popup.x, large.x + (large.width - popup.width) / 2);
        assert_eq!(popup.y, large.y + (large.height - popup.height) / 2);
        assert_eq!(help_max_scroll(large, &app), 0);
        let text = draw(&app, 300, 64);
        assert!(text.contains("CURRENT PANEL") && text.contains("HELP-TAIL"));

        for (width, height) in [(120, 30), (80, 12), (60, 16), (36, 10)] {
            let area = Rect::new(0, 0, width, height);
            let popup = help_area(area, &app);
            assert!(popup.x >= 2 && popup.right() <= width - 2);
            assert!(popup.y >= 1 && popup.bottom() < height);
            app.help_scroll = help_max_scroll(area, &app);
            let text = draw(&app, width, height);
            assert!(text.contains("HELP-TAIL"));
            assert!(text.contains("?/Esc close | q leave"));
        }
        // A stale scroll position must not hide content after enlarging the window.
        let text = draw(&app, 300, 64);
        assert!(text.contains("CURRENT PANEL") && text.contains("HELP-TAIL"));
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
        assert!(
            diagnostics(&app)
                .iter()
                .any(|line| line.to_string().contains("GAP"))
        );
        assert_eq!(log_height(area, &app), 6);
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
    #[test]
    fn trimming_a_retrievable_client_window_is_not_reported_as_log_loss() {
        let mut app = App::new();
        app.logs.local_evicted = 100;
        assert!(!status_line(&app).to_string().contains("LOG LOSS"));
        app.logs.dropped = 1;
        assert!(status_line(&app).to_string().contains("LOG LOSS"));
    }
}
