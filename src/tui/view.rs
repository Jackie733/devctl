use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Cell, Clear, Padding, Paragraph, Row, Table},
};

use super::app::{App, Focus};
use crate::{
    logs::{Record, Source},
    manager::ServiceState,
};

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

fn search_text(app: &App) -> Option<&str> {
    app.search_input
        .as_deref()
        .or_else(|| app.logs.search.as_ref().map(|s| s.text.as_str()))
        .filter(|text| !text.is_empty())
}

fn log_body(
    text: &str,
    needle: Option<&str>,
    base: Style,
    current: bool,
    selected: bool,
) -> Vec<Span<'static>> {
    let Some(needle) = needle.filter(|s| !s.is_empty()) else {
        return vec![Span::styled(clean(text), base)];
    };
    let matched = if selected {
        base.fg(Color::Yellow)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else {
        base.fg(Color::Black)
            .bg(if current {
                Color::LightYellow
            } else {
                Color::Yellow
            })
            .add_modifier(if current {
                Modifier::BOLD | Modifier::UNDERLINED
            } else {
                Modifier::BOLD
            })
    };
    let mut spans = Vec::new();
    let mut end = 0;
    for (start, found) in text.match_indices(needle) {
        if start > end {
            spans.push(Span::styled(clean(&text[end..start]), base));
        }
        spans.push(Span::styled(clean(found), matched));
        end = start + found.len();
    }
    if end < text.len() {
        spans.push(Span::styled(clean(&text[end..]), base));
    }
    spans
}

fn timestamp(record: &Record, width: u16) -> String {
    if width < 100 || record.captured_ms == 0 {
        return String::new();
    }
    let Ok(seconds) = nix::libc::time_t::try_from(record.captured_ms / 1000) else {
        return "??:??:?? ".into();
    };
    // localtime_r uses the client's system timezone (or TZ override), including
    // the daylight-saving offset at capture time. No process-wide TZ mutation.
    let mut local = std::mem::MaybeUninit::<nix::libc::tm>::uninit();
    // SAFETY: both pointers are valid; local is read only after a successful call.
    let local = unsafe {
        if nix::libc::localtime_r(&seconds, local.as_mut_ptr()).is_null() {
            return "??:??:?? ".into();
        }
        local.assume_init()
    };
    format!(
        "{:02}:{:02}:{:02} ",
        local.tm_hour, local.tm_min, local.tm_sec
    )
}

fn record_label(app: &App, record: &Record) -> String {
    let source = match record.source {
        Source::Stdout => "out",
        Source::Stderr => "err",
    };
    let pointer = if app.logs.selection.is_some() || app.logs.reading.is_some() {
        if app.logs.selection.map(|s| s.head).or(app.logs.reading) == Some(record.sequence) {
            "> "
        } else {
            "  "
        }
    } else {
        ""
    };
    format!("{pointer}[{source} #{}] ", record.instance)
}

pub fn reveal_search_match(app: &mut App, area: Rect) {
    let pane = layout(area, app).logs;
    let content = log_areas(pane)[1];
    app.logs.reveal_match(usize::from(content.height).max(1));
    let Some(search) = &app.logs.search else {
        return;
    };
    let Some(record) = app
        .logs
        .records
        .iter()
        .find(|r| Some(r.sequence) == search.matched)
    else {
        return;
    };
    let Some(offset) = record.text.find(&search.text) else {
        return;
    };
    let prefix = Line::from(format!(
        "{}{}",
        timestamp(record, pane.width),
        record_label(app, record)
    ))
    .width();
    let start = prefix + Line::from(clean(&record.text[..offset])).width();
    let end = start + Line::from(clean(&search.text)).width();
    let left = usize::from(app.logs.horizontal);
    let width = usize::from(content.width).max(1);
    if start < left || end > left.saturating_add(width) {
        app.logs.horizontal = start.saturating_sub(width / 4).min(usize::from(u16::MAX)) as u16;
    }
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
        lines.push(help_action("Saving", &clean(&info.state)));
        lines.push(help_action("Session", &clean(&info.session)));
        lines.push(help_action("Directory", &clean(&info.directory)));
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
    } else if app.feedback_visible() {
        if app.pending.is_some() || app.copying {
            text.push_str(spinner(app));
            text.push(' ');
        } else if app.feedback_succeeded() {
            text.push_str(if app.appearance.ascii { "OK " } else { "✓ " });
        }
        text.push_str(&clean(&app.feedback));
    }
    Line::styled(
        text,
        if flags.is_empty() && app.feedback_succeeded() {
            style.fg(Color::Green)
        } else {
            style
        },
    )
}

fn spinner(app: &App) -> &'static str {
    if app.appearance.reduced_motion {
        return "~";
    }
    if app.appearance.ascii {
        return ["|", "/", "-", "\\"][app.animation_frame % 4];
    }
    ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"][app.animation_frame % 10]
}

fn state_icon(app: &App, id: &str) -> &'static str {
    if !app.connected {
        return "?";
    }
    match app.services[id].state {
        ServiceState::Stopped => {
            if app.appearance.ascii {
                "-"
            } else {
                "○"
            }
        }
        ServiceState::Running => {
            if app.appearance.ascii {
                "*"
            } else {
                "●"
            }
        }
        ServiceState::Starting | ServiceState::Stopping => spinner(app),
        ServiceState::Failed => {
            if app.appearance.ascii {
                "x"
            } else {
                "×"
            }
        }
    }
}

fn shortcuts(text: String) -> Line<'static> {
    Line::from(
        text.split_inclusive(' ')
            .map(|word| {
                let key = word.trim();
                let style = if matches!(
                    key,
                    "s/x"
                        | "j/k"
                        | "Enter"
                        | "a"
                        | "A"
                        | "X"
                        | "Q"
                        | "/"
                        | "n/N"
                        | "v"
                        | "p"
                        | "G"
                        | "g"
                        | "y"
                        | "Esc"
                        | "?"
                        | "q"
                ) {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                Span::styled(word.to_owned(), style)
            })
            .collect::<Vec<_>>(),
    )
}

// Fit whole actions in priority order; never display a clipped key or label.
fn footer(app: &App, width: u16) -> Line<'static> {
    let items: &[&str] = if app.goto_top {
        &["g top", "Esc cancel"]
    } else if app.logs.selection.is_some() {
        &["y copy", "Esc", "j/k extend"]
    } else if app.focus == Focus::Projects {
        &[
            "s/x start/stop",
            "Enter logs",
            "a add",
            "j/k select",
            "A default",
            "X stop ALL",
            "Q end",
        ]
    } else if app.search_input.is_some() {
        &["Enter search", "Esc cancel"]
    } else if app.logs.search.is_some() {
        &["n/N match", "Esc clear", "G follow", "/ search"]
    } else {
        &["/ search", "v select", "G follow", "p pause", "j/k move"]
    };
    let mut text = String::new();
    for item in items {
        let candidate = if text.is_empty() {
            item.to_string()
        } else {
            format!("{text} | {item}")
        };
        // One blank cell separates contextual actions from the reserved global help.
        if Line::from(candidate.as_str()).width() >= usize::from(width) {
            break;
        }
        text = candidate;
    }
    shortcuts(text)
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
        ServiceState::Stopped => (
            "STOPPED",
            "Retained logs; process stopped.",
            Color::DarkGray,
        ),
        ServiceState::Starting => (
            "STARTING",
            "Waiting for process state; earlier logs may remain.",
            Color::Yellow,
        ),
        ServiceState::Running => (
            "RUNNING",
            "Process alive; header shows latest run.",
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
        .title_style(if active {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        })
}

pub fn render(frame: &mut Frame, app: &App) {
    render_content(frame, app);
    app.appearance.apply(frame.buffer_mut());
}

fn render_content(frame: &mut Frame, app: &App) {
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
    let connection_icon = if app.connected {
        if app.appearance.ascii { "*" } else { "●" }
    } else if app.connection_error.is_none() && app.workspace_id.is_none() {
        spinner(app)
    } else {
        "?"
    };
    let mut header = vec![Line::from(vec![
        Span::styled(
            format!("devctl {}", env!("CARGO_PKG_VERSION")),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | "),
        Span::styled(
            format!("{connection} {connection_icon}"),
            Style::default().fg(if app.connected {
                Color::Green
            } else {
                Color::Yellow
            }),
        ),
        Span::styled(
            format!(" | workspace {identity}"),
            Style::default().fg(Color::DarkGray),
        ),
    ])];
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
    let [context_area, global] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(17)]).areas(screen.footer);
    frame.render_widget(
        Paragraph::new(footer(app, context_area.width)),
        context_area,
    );
    frame.render_widget(Paragraph::new(shortcuts("? help | q leave".into())), global);
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
        let status_style = Style::default().fg(if selected && color == Color::DarkGray {
            Color::Gray
        } else {
            color
        });
        Row::new(vec![
            Cell::from(format!(
                "{}{}",
                if selected {
                    if app.appearance.ascii { "> " } else { "▸ " }
                } else {
                    "  "
                },
                clean(id)
            )),
            Cell::from(Line::styled(
                format!("{} {status}", state_icon(app, id)),
                status_style,
            )),
        ])
        .style(if selected {
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        })
    });
    frame.render_widget(
        Table::new(rows, [Constraint::Min(0), Constraint::Length(10)]),
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
        .map(|instance| format!(" | latest #{instance}"))
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
    let mode_color = if app.logs.selection.is_some() {
        Color::Magenta
    } else if app.logs.paused {
        Color::Yellow
    } else {
        Color::Cyan
    };
    let loading = app.connected
        && (app.logs.requested.is_some()
            || app
                .logs
                .search
                .as_ref()
                .is_some_and(|search| search.running));
    let block = panel(title, app.focus == Focus::Logs).title_bottom(Line::from(vec![
        Span::styled(
            position,
            Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!("{history}{offset}")),
        Span::styled(
            if loading {
                format!(" | {} loading", spinner(app))
            } else {
                String::new()
            },
            Style::default().fg(Color::Yellow),
        ),
    ]));
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
            format!("{} Reading recent output...", spinner(app))
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
            let color = match record.source {
                Source::Stdout => Color::Reset,
                Source::Stderr => Color::LightRed,
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
            let mut spans = vec![
                Span::styled(
                    timestamp(record, area.width),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    record_label(app, record),
                    if selected {
                        row_style
                    } else if head && app.focus == Focus::Logs {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ),
            ];
            spans.extend(log_body(
                &record.text,
                search_text(app),
                if selected {
                    row_style
                } else {
                    Style::default().fg(color)
                },
                app.logs
                    .search
                    .as_ref()
                    .is_some_and(|s| s.matched == Some(record.sequence)),
                selected,
            ));
            spans.push(Span::styled(
                if record.truncated { " [truncated]" } else { "" },
                Style::default().fg(Color::Yellow),
            ));
            Line::from(spans).style(row_style)
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

// Wrap at word boundaries when possible, but also split long IDs/paths by terminal
// columns. Keep graphemes and their styles intact; measurement and drawing share
// these exact rows so narrow windows cannot hide the end of a diagnostic.
fn wrapped(lines: Vec<Line<'static>>, width: u16) -> Vec<Line<'static>> {
    if width == 0 {
        return Vec::new();
    }
    let mut result = Vec::new();
    for line in lines {
        let glyphs: Vec<_> = line.styled_graphemes(Style::default()).collect();
        if glyphs.is_empty() {
            result.push(line.clone());
            continue;
        }
        let mut start = 0;
        while start < glyphs.len() {
            let mut end = start;
            let mut columns = 0;
            let mut space = None;
            while end < glyphs.len() {
                let size = Span::raw(glyphs[end].symbol).width();
                if columns + size > usize::from(width) && end > start {
                    break;
                }
                columns += size;
                if glyphs[end].symbol.chars().all(char::is_whitespace) {
                    space = Some(end);
                }
                end += 1;
            }
            if end < glyphs.len()
                && !glyphs[end].symbol.chars().all(char::is_whitespace)
                && let Some(boundary) = space.filter(|&index| index > start)
            {
                end = boundary;
            }
            let mut spans: Vec<Span<'static>> = Vec::new();
            for glyph in &glyphs[start..end] {
                if let Some(last) = spans.last_mut()
                    && last.style == glyph.style
                {
                    last.content.to_mut().push_str(glyph.symbol);
                } else {
                    spans.push(Span::styled(glyph.symbol.to_owned(), glyph.style));
                }
            }
            result.push(Line::from(spans));
            start = end;
            while start < glyphs.len() && glyphs[start].symbol.chars().all(char::is_whitespace) {
                start += 1;
            }
        }
    }
    result
}

fn modal_style() -> Style {
    Style::default().fg(Color::White).bg(Color::Black)
}

fn modal_block(title: &'static str, accent: Color) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent))
        .title(Span::styled(
            title,
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ))
        .style(modal_style())
        .padding(Padding::new(2, 2, 1, 1))
}

// De-emphasize the whole main view, including search/selection backgrounds, then
// clear a one-cell gutter. Log output must never meet the dialog border directly.
fn modal_backdrop(frame: &mut Frame, area: Rect) {
    let screen = frame.area();
    frame.buffer_mut().set_style(
        screen,
        Style::default()
            .fg(Color::DarkGray)
            .bg(Color::Reset)
            .remove_modifier(Modifier::all()),
    );
    let gutter = Rect::new(
        area.x.saturating_sub(1).max(screen.x),
        area.y.saturating_sub(1).max(screen.y),
        area.width.saturating_add(2),
        area.height.saturating_add(2),
    )
    .intersection(screen);
    frame.render_widget(Clear, gutter);
    frame.render_widget(Block::default().style(modal_style()), gutter);
}

fn help_action(keys: &str, action: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{keys:<18}"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(action.to_owned()),
    ])
}

fn heading(text: &str) -> Line<'static> {
    Line::styled(
        text.to_owned(),
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )
}

fn note(text: &str) -> Line<'static> {
    Line::styled(text.to_owned(), Style::default().fg(Color::Gray))
}

fn shutdown_lines(app: &App, width: u16) -> Vec<Line<'static>> {
    let confirm = Span::styled(
        "y: confirm",
        Style::default()
            .fg(Color::LightRed)
            .add_modifier(Modifier::BOLD),
    );
    let cancel = Span::styled(
        "n / Esc: cancel",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );
    let mut lines = vec![
        heading(&format!("Stop ALL {} projects?", app.services.len())),
        Line::from("All running projects will stop; the background workspace will end."),
        Line::default(),
    ];
    if usize::from(width) >= confirm.width() + cancel.width() + 4 {
        lines.push(Line::from(vec![confirm, Span::raw("    "), cancel]));
    } else {
        lines.extend([Line::from(confirm), Line::from(cancel)]);
    }
    lines.push(note("q / Ctrl-C: leave only; services keep running"));
    wrapped(lines, width)
}

fn shutdown_block() -> Block<'static> {
    modal_block(" End workspace ", Color::LightRed)
}

fn shutdown_area(area: Rect, app: &App) -> Rect {
    let width = area.width.saturating_sub(4).min(72);
    let inner = shutdown_block().inner(Rect::new(0, 0, width, area.height));
    let height = shutdown_lines(app, inner.width).len().saturating_add(4);
    popup(
        area,
        width,
        (height.min(usize::from(u16::MAX)) as u16).min(area.height.saturating_sub(2)),
    )
}

pub fn can_confirm(area: Rect, app: &App) -> bool {
    let inner = shutdown_block().inner(shutdown_area(area, app));
    usable(area) && shutdown_lines(app, inner.width).len() <= usize::from(inner.height)
}

fn help_lines(app: &App) -> Vec<Line<'static>> {
    let mut lines = vec![heading("CURRENT PANEL")];
    if app.logs.selection.is_some() {
        lines.push(help_action(
            "j/k / Up/Down",
            "Extend or shrink selected lines",
        ));
        lines.push(help_action("y / Esc", "Copy selection / cancel selection"));
        lines.push(help_action(
            "gg / G",
            "Selection endpoint to first / last line",
        ));
        lines.push(note("Cancel selection before panel or service controls."));
    } else {
        let (motion, ends) = match app.focus {
            Focus::Projects => ("Select project", "First / last project"),
            Focus::Logs => ("Move log cursor", "Oldest retained / follow latest logs"),
        };
        lines.push(help_action("j/k / Up/Down", motion));
        lines.push(help_action("gg / G", ends));
        lines.push(match app.focus {
            Focus::Projects => help_action("Enter", "Open selected project's logs"),
            Focus::Logs => help_action("Esc", "Return to projects (clear search first)"),
        });
        lines.push(help_action(
            "h/l / Tab",
            "Focus panel; Shift-Tab also switches",
        ));
        lines.push(help_action("1 / 0", "Focus projects / logs"));
    }
    lines.extend([
        Line::default(),
        heading("PROJECT & WORKSPACE"),
        help_action("s / x", "Start / stop selected project"),
        help_action("a", "Add a project: directory, command, save"),
        help_action("A", "Start defaults (autostart = true)"),
        help_action("X", "Stop ALL projects; keep background"),
        help_action("Q", "Confirm ending the entire workspace"),
        help_action("r", "Sync existing background"),
        note("Running means process alive, not business readiness."),
        Line::default(),
        heading("READ LOGS"),
        help_action("Ctrl-u/d", "Scroll half a page"),
        help_action("PageUp/Down", "Scroll a page; edges load history"),
        help_action("Home / End", "Oldest retained / follow latest logs"),
        help_action("p", "Pause / resume following"),
        help_action("H/L / Left/Right", "Pan long lines horizontally"),
        note("FOLLOW/READ describes scrolling; the header shows service state."),
        note("out/err = stdout/stderr; # identifies a run; header shows latest #."),
        note("Log times use your local timezone (system or TZ)."),
        Line::default(),
        heading("SEARCH & COPY"),
        help_action("/ then Enter", "Highlight literal text and find a match"),
        help_action("n / N", "Next / previous matching line"),
        help_action("Esc", "Clear search; keep surrounding logs"),
        help_action("v then j/k", "Select lines from the log cursor"),
        help_action("y / Esc", "In selection: copy / cancel"),
        note("/ starts a fresh search snapshot; context stays visible."),
        note("Local WSL: clip.exe; local Mac: pbcopy; elsewhere/SSH: OSC 52."),
        note("Check copy feedback and paste to verify. Failures keep selection."),
        note("Selected logs stay anchored; eviction cancels selection."),
        Line::default(),
        heading("APPEARANCE"),
        note("DEVCTL_TUI_THEME=dark|light|terminal (default: dark)"),
        note("DEVCTL_TUI_REDUCED_MOTION=1: static waiting indicators"),
        note("DEVCTL_TUI_ASCII=1: plain status symbols; NO_COLOR=1: monochrome"),
        Line::default(),
        heading("LEAVING"),
        help_action("q / Ctrl-C", "Leave interface; services keep running"),
        Line::default(),
        heading("CURRENT DETAILS"),
        Line::from(selected_summary(app)),
    ]);
    lines.extend(diagnostics(app));
    lines
}

fn help_block() -> Block<'static> {
    modal_block(" Help ", Color::Cyan)
}

fn help_area(area: Rect, app: &App) -> Rect {
    let width = area.width.saturating_sub(4).min(80);
    let inner = help_block().inner(Rect::new(0, 0, width, area.height));
    let rows = wrapped(help_lines(app), inner.width).len();
    // Borders, vertical padding and the two fixed footer rows occupy six rows.
    let height = rows.saturating_add(6).min(usize::from(u16::MAX)) as u16;
    popup(area, width, height.min(area.height.saturating_sub(2)))
}

pub fn help_height(area: Rect, app: &App) -> u16 {
    help_block()
        .inner(help_area(area, app))
        .height
        .saturating_sub(2)
        .max(1)
}

pub fn help_max_scroll(area: Rect, app: &App) -> u16 {
    let inner = help_block().inner(help_area(area, app));
    let height = usize::from(inner.height.saturating_sub(2));
    let count = wrapped(help_lines(app), inner.width).len();
    count.saturating_sub(height).min(usize::from(u16::MAX)) as u16
}

pub fn onboarding_usable(area: Rect) -> bool {
    area.width >= 50 && area.height >= 12
}

fn onboarding_rows(area: Rect, app: &App) -> (Rect, Vec<Line<'static>>, u16) {
    let area = popup(
        area,
        area.width.saturating_sub(4).min(96),
        area.height.saturating_sub(2).min(28),
    );
    let inner = modal_block(" Add project ", Color::Cyan).inner(area);
    let dialog = app.add.as_ref().expect("open dialog");
    let lines = wrapped(
        dialog
            .lines(usize::from(inner.height))
            .into_iter()
            .map(|line| Line::from(clean(&line)))
            .collect(),
        inner.width,
    );
    let height = lines
        .len()
        .saturating_add(4)
        .max(8)
        .min(usize::from(area.height)) as u16;
    let area = popup(area, area.width, height);
    let inner = modal_block(" Add project ", Color::Cyan).inner(area);
    let max = lines
        .len()
        .saturating_sub(usize::from(inner.height))
        .min(u16::MAX as usize) as u16;
    (area, lines, max)
}
pub fn onboarding_can_save(area: Rect, app: &App) -> bool {
    if !onboarding_usable(area) {
        return false;
    }
    let (_, _, max) = onboarding_rows(area, app);
    app.add.as_ref().is_some_and(|dialog| dialog.scroll >= max)
}

fn overlay(frame: &mut Frame, app: &App) {
    if let Some(dialog) = &app.add {
        let (area, lines, max) = onboarding_rows(frame.area(), app);
        modal_backdrop(frame, area);
        let block = modal_block(" Add project ", Color::Cyan).title_bottom(
            if matches!(dialog.step, super::onboarding::Step::Review(_)) {
                " Up/Down: scroll | Enter: save | e: edit | Esc: back "
            } else {
                " Ctrl-C: leave "
            },
        );
        let lines = if onboarding_usable(frame.area()) {
            lines
        } else {
            vec![
                heading("Resize to at least 50 x 12."),
                note("Esc: back/cancel | Ctrl-C: leave"),
            ]
        };
        frame.render_widget(
            Paragraph::new(lines)
                .scroll((
                    if dialog.text_input() {
                        max
                    } else {
                        dialog.scroll.min(max)
                    },
                    0,
                ))
                .style(modal_style())
                .block(block),
            area,
        );
    } else if app.confirm_shutdown {
        let area = shutdown_area(frame.area(), app);
        modal_backdrop(frame, area);
        let block = shutdown_block();
        let inner = block.inner(area);
        let text = if can_confirm(frame.area(), app) {
            shutdown_lines(app, inner.width)
        } else {
            wrapped(
                vec![
                    heading("Enlarge window to confirm."),
                    Line::from("n / Esc: cancel | q: leave"),
                ],
                inner.width,
            )
        };
        frame.render_widget(Paragraph::new(text).style(modal_style()).block(block), area);
    } else if app.help {
        let area = help_area(frame.area(), app);
        modal_backdrop(frame, area);
        let block = help_block();
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let [body, footer] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(2)]).areas(inner);
        let max = help_max_scroll(frame.area(), app);
        let scroll = app.help_scroll.min(max);
        let lines = wrapped(help_lines(app), body.width);
        let count = lines.len();
        frame.render_widget(
            Paragraph::new(lines)
                .style(modal_style())
                .scroll((scroll, 0)),
            body,
        );
        let navigation = if app.goto_top {
            "g ... | gg first"
        } else {
            "j/k scroll | gg/G ends"
        };
        let mut position = navigation.to_owned();
        let counter = format!(
            "{}-{}/{}",
            usize::from(scroll) + 1,
            (usize::from(scroll) + usize::from(body.height)).min(count),
            count
        );
        if usize::from(footer.width) >= position.len() + counter.len() + 2 {
            position
                .push_str(&" ".repeat(usize::from(footer.width) - position.len() - counter.len()));
            position.push_str(&counter);
        }
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled(position, Style::default().fg(Color::Gray)),
                Line::styled("?/Esc close | q leave", Style::default().fg(Color::Cyan)),
            ])
            .style(modal_style()),
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
    fn timestamps_follow_client_timezone() {
        const EXPECTED: &str = "DEVCTL_TEST_LOG_TIMES";
        if let Ok(expected) = std::env::var(EXPECTED) {
            let mut record = Record {
                captured_ms: 1_768_439_758_000, // 2026-01-15 01:15:58 UTC
                sequence: 1,
                instance: 1,
                source: Source::Stdout,
                text: String::new(),
                truncated: false,
            };
            let winter = timestamp(&record, 100);
            assert_eq!(timestamp(&record, 99), "");
            record.captured_ms = 1_784_078_158_000; // 2026-07-15 01:15:58 UTC
            let summer = timestamp(&record, 100);
            assert_eq!(format!("{winter}|{summer}"), expected);
            record.captured_ms = 0; // Older supervisors can omit capture time.
            assert_eq!(timestamp(&record, 100), "");
            return;
        }
        // Isolate TZ in child processes instead of changing the environment of
        // concurrent Rust tests. Include date rollover and capture-time DST.
        for (zone, expected) in [
            ("UTC0", "01:15:58 |01:15:58 "),
            ("CST-8", "09:15:58 |09:15:58 "),
            ("NPT-5:45", "07:00:58 |07:00:58 "),
            ("EST5EDT,M3.2.0,M11.1.0", "20:15:58 |21:15:58 "),
        ] {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "tui::view::tests::timestamps_follow_client_timezone",
                ])
                .env("TZ", zone)
                .env(EXPECTED, expected)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{zone}: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    #[test]
    fn selection_keeps_status_colors_readable_in_wide_and_narrow_views() {
        let mut app = App::new();
        app.apply_status(Response {
            workspace_id: "one".into(),
            services: [
                (
                    "api".into(),
                    ServiceStatus {
                        state: ServiceState::Running,
                        ..ServiceStatus::default()
                    },
                ),
                ("web".into(), ServiceStatus::default()),
            ]
            .into(),
            ..Response::default()
        });
        let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let running = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .find(|cell| {
                cell.symbol() == "●" && cell.bg == app.appearance.background(Color::DarkGray)
            })
            .unwrap();
        assert_eq!(running.fg, app.appearance.foreground(Color::Green));
        app.select(1);
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let stopped = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .find(|cell| cell.symbol() == "○")
            .unwrap();
        assert_ne!(stopped.fg, stopped.bg);
        for (width, height) in [(120, 30), (80, 12), (36, 10)] {
            let text = draw(&app, width, height);
            assert!(text.contains("○ Stopped") && text.contains("q leave"));
        }
        app.disconnect("lost".into());
        let text = draw(&app, 120, 30);
        assert!(text.contains("? Unknown") && !text.contains("● Running"));
    }

    #[test]
    fn expired_success_keeps_diagnostics_and_new_failures_visible() {
        let mut app = App::new();
        app.apply_status(Response {
            workspace_id: "one".into(),
            ..Response::default()
        });
        app.success("Copied 12 lines; verify paste.".into());
        assert!(status_line(&app).to_string().contains("✓ Copied"));
        app.tick(std::time::Instant::now() + std::time::Duration::from_secs(3));
        assert!(!status_line(&app).to_string().contains("Copied"));
        assert!(
            diagnostics(&app)
                .iter()
                .any(|line| line.to_string().contains("verify paste"))
        );
        app.logs.gap = true;
        app.feedback = "Clipboard copy failed: denied.".into();
        let text = status_line(&app).to_string();
        assert!(text.contains("GAP") && text.contains("copy failed"));
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
        assert!(stopped.contains("Logs · STOPPED: web | latest #5"));
        assert!(stopped.contains("STOPPED | Retained logs"));
        for instance in [3, 5, 6] {
            assert!(stopped.contains(&format!("[out #{instance}]")));
        }
        assert!(!stopped.contains(" old") && !stopped.contains("seq="));
        let labels: Vec<_> = app
            .logs
            .records
            .iter()
            .map(|r| record_label(&app, r))
            .collect();
        app.services.get_mut("web").unwrap().instance = Some(7);
        assert_eq!(
            labels,
            app.logs
                .records
                .iter()
                .map(|r| record_label(&app, r))
                .collect::<Vec<_>>()
        );
        app.connected = false;
        assert_eq!(
            labels,
            app.logs
                .records
                .iter()
                .map(|r| record_label(&app, r))
                .collect::<Vec<_>>()
        );
        app.connected = true;
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
        assert_eq!(
            buffer[(log_area.x + 1, log_area.y + 2)].bg,
            app.appearance.background(Color::DarkGray)
        );
        assert_eq!(
            buffer[(log_area.x + 1, log_area.y + 3)].bg,
            app.appearance.background(Color::Blue)
        );
        assert_ne!(
            buffer[(log_area.x + 1, log_area.y + 4)].bg,
            app.appearance.background(Color::Blue)
        );
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
        let inner_width = help_block().inner(popup).width;
        assert_eq!(
            usize::from(popup.height),
            wrapped(help_lines(&app), inner_width).len() + 6
        );
        assert!(popup.height < large.height - 2);
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
        assert!(wrapped.iter().flat_map(|line| &line.spans).any(|span| {
            span.content.contains("END-OF-DIAGNOSTIC") && span.style.fg == Some(Color::LightRed)
        }));
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
    fn modal_wrapping_keeps_words_and_key_colors() {
        let lines = wrapped(
            vec![Line::from(vec![
                Span::styled("n / N", Style::default().fg(Color::Cyan)),
                Span::raw("  Next / previous matching line"),
            ])],
            20,
        );
        assert_eq!(
            lines.iter().map(Line::to_string).collect::<Vec<_>>(),
            ["n / N  Next /", "previous matching", "line"]
        );
        assert_eq!(lines[0].spans[0].style.fg, Some(Color::Cyan));
        let lines = wrapped(vec![heading("one two three")], 7);
        assert_eq!(
            lines.iter().map(Line::to_string).collect::<Vec<_>>(),
            ["one two", "three"]
        );
        assert!(lines.iter().flat_map(|line| &line.spans).all(|span| {
            span.style.fg == Some(Color::Yellow) && span.style.add_modifier.contains(Modifier::BOLD)
        }));
    }

    #[test]
    fn modals_mask_busy_logs_and_leave_a_clear_gutter_on_every_redraw() {
        let mut app = searched_app();
        for record in &mut app.logs.records {
            record.text = "needle BACKGROUND 中文日志👩‍💻 ".repeat(16);
        }
        let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
        for confirm in [false, true] {
            app.help = !confirm;
            app.confirm_shutdown = confirm;
            let screen = Rect::new(0, 0, 120, 30);
            let area = if confirm {
                shutdown_area(screen, &app)
            } else {
                help_area(screen, &app)
            };
            for _ in 0..2 {
                terminal.draw(|frame| render(frame, &app)).unwrap();
                let buffer = terminal.backend().buffer();
                assert_eq!(buffer[(area.x, area.y)].symbol(), "╭");
                assert_eq!(
                    buffer[(area.x, area.y)].fg,
                    if confirm {
                        app.appearance.foreground(Color::LightRed)
                    } else {
                        app.appearance.foreground(Color::Cyan)
                    }
                );
                for y in area.y - 1..=area.bottom() {
                    for x in area.x - 1..=area.right() {
                        let cell = &buffer[(x, y)];
                        assert_eq!(cell.bg, app.appearance.background(Color::Black));
                        if !area.contains((x, y).into()) {
                            assert_eq!(cell.symbol(), " ");
                        }
                    }
                }
                // Outside the gutter, original text remains as a subdued backdrop.
                assert_eq!(
                    buffer[(0, 0)].fg,
                    app.appearance.foreground(Color::DarkGray)
                );
                let text: String = (area.y..area.bottom())
                    .flat_map(|y| (area.x..area.right()).map(move |x| buffer[(x, y)].symbol()))
                    .collect();
                assert!(!text.contains("BACKGROUND"));
                assert!(text.contains(if confirm {
                    "y: confirm"
                } else {
                    "?/Esc close | q leave"
                }));
                app.logs
                    .records
                    .back_mut()
                    .unwrap()
                    .text
                    .push_str(" new output");
            }
        }
        app.help = false;
        app.confirm_shutdown = false;
        terminal.draw(|frame| render(frame, &app)).unwrap();
        assert!(
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .any(|cell| cell.bg == app.appearance.background(Color::LightYellow))
        );
        assert!(draw(&app, 120, 30).contains("BACKGROUND"));
    }

    #[test]
    fn shutdown_never_offers_confirmation_when_its_scope_or_actions_are_clipped() {
        let mut app = App::new();
        app.confirm_shutdown = true;
        for (width, height) in [(36, 10), (80, 10), (36, 20), (80, 12), (120, 30)] {
            let screen = Rect::new(0, 0, width, height);
            let text = draw(&app, width, height);
            assert_eq!(text.contains("y: confirm"), can_confirm(screen, &app));
            if can_confirm(screen, &app) {
                assert!(text.contains("Stop ALL 0 projects?"));
                assert!(text.contains("n / Esc: cancel"));
            } else {
                assert!(text.contains("Enlarge window to confirm."));
            }
        }
    }
    #[test]
    fn trimming_a_retrievable_client_window_is_not_reported_as_log_loss() {
        let mut app = App::new();
        app.logs.local_evicted = 100;
        assert!(!status_line(&app).to_string().contains("LOG LOSS"));
        app.logs.dropped = 1;
        assert!(status_line(&app).to_string().contains("LOG LOSS"));
    }
    fn searched_app() -> App {
        let mut app = App::new();
        app.apply_status(Response {
            workspace_id: "one".into(),
            services: [("web".into(), ServiceStatus::default())].into(),
            ..Response::default()
        });
        app.focus(Focus::Logs);
        app.apply_logs(crate::logs::Batch {
            records: (1..=40)
                .map(|sequence| Record {
                    captured_ms: 0,
                    sequence,
                    instance: 1,
                    source: Source::Stdout,
                    text: if sequence == 20 {
                        "前 needle 中 needle".into()
                    } else if sequence == 22 {
                        "other needle".into()
                    } else {
                        format!("context-{sequence}")
                    },
                    truncated: false,
                })
                .collect(),
            oldest: Some(1),
            latest: 40,
            ..crate::logs::Batch::default()
        });
        app.logs.start_search("needle".into(), false, true, 20);
        let record = app.logs.records[19].clone();
        app.apply_log_page(
            super::super::app::LogQuery::Search(0),
            crate::logs::Batch {
                records: vec![record],
                ..crate::logs::Batch::default()
            },
        );
        reveal_search_match(&mut app, Rect::new(0, 0, 120, 30));
        app
    }

    #[test]
    fn search_highlights_all_visible_occurrences_preserves_context_and_selection_styles() {
        let mut app = searched_app();
        let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
        terminal.draw(|f| render(f, &app)).unwrap();
        let cells = &terminal.backend().buffer().content;
        assert_eq!(
            cells
                .iter()
                .filter(|c| c.bg == app.appearance.background(Color::LightYellow))
                .count(),
            12
        );
        assert_eq!(
            cells
                .iter()
                .filter(|c| c.bg == app.appearance.background(Color::Yellow))
                .count(),
            6
        );
        let text = draw(&app, 120, 30);
        assert!(
            text.contains("context-19")
                && text.contains("context-21")
                && text.contains("other needle")
        );
        app.logs.begin_selection(20);
        terminal.draw(|f| render(f, &app)).unwrap();
        let cells = &terminal.backend().buffer().content;
        assert_eq!(
            cells
                .iter()
                .filter(|c| c.bg == app.appearance.background(Color::Blue)
                    && c.fg == app.appearance.foreground(Color::Yellow)
                    && c.modifier.contains(Modifier::UNDERLINED))
                .count(),
            12
        );
        assert_eq!(app.logs.selected_text().unwrap().0, "前 needle 中 needle\n");
        app.logs.selection = None;
        app.logs.clear_search();
        terminal.draw(|f| render(f, &app)).unwrap();
        assert!(!terminal.backend().buffer().content.iter().any(|c| {
            [Color::Yellow, Color::LightYellow]
                .map(|color| app.appearance.background(color))
                .contains(&c.bg)
        }));
        app.search_input = Some("needle".into());
        terminal.draw(|f| render(f, &app)).unwrap();
        assert_eq!(
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .filter(|c| c.bg == app.appearance.background(Color::Yellow))
                .count(),
            18
        );
    }

    #[test]
    fn a_search_hit_on_a_long_unicode_line_scrolls_into_view() {
        let mut app = searched_app();
        app.logs.records[19].text = format!("{} needle 文", "界".repeat(80));
        reveal_search_match(&mut app, Rect::new(0, 0, 120, 30));
        assert!(app.logs.horizontal > 0);
        assert!(draw(&app, 120, 30).contains("needle 文"));
        // Matching uses stored text, so a tab rendered as spaces cannot create a false hit.
        let spans = log_body(
            "pre\tneedle",
            Some("pre    needle"),
            Style::default(),
            false,
            false,
        );
        assert!(spans.iter().all(|s| s.style.bg.is_none()));
        assert_eq!(
            spans.iter().map(|s| s.content.as_ref()).collect::<String>(),
            "pre    needle"
        );
    }
    #[test]
    fn state_changes_never_invert_or_flash_the_selected_background() {
        let mut app = searched_app();
        app.focus(Focus::Projects);
        let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
        for state in [
            ServiceState::Starting,
            ServiceState::Running,
            ServiceState::Stopping,
            ServiceState::Stopped,
            ServiceState::Failed,
        ] {
            let mut services = app.services.clone();
            services.get_mut("web").unwrap().state = state;
            app.apply_status(Response {
                workspace_id: "one".into(),
                services,
                ..Response::default()
            });
            for millis in [0, 100, 300, 500] {
                app.tick(std::time::Instant::now() + std::time::Duration::from_millis(millis));
                terminal.draw(|frame| render(frame, &app)).unwrap();
                let row = terminal.backend().buffer();
                for x in 1..31 {
                    let cell = &row[(x, 2)];
                    assert_eq!(cell.bg, app.appearance.background(Color::DarkGray));
                    assert!(!cell.modifier.intersects(
                        Modifier::REVERSED | Modifier::SLOW_BLINK | Modifier::RAPID_BLINK
                    ));
                }
            }
        }
    }

    #[test]
    fn footer_fits_complete_actions_and_preserves_global_help() {
        let mut app = searched_app();
        for focus in [Focus::Projects, Focus::Logs] {
            app.focus(focus);
            for (width, height) in [(120, 30), (80, 24), (60, 18), (36, 10)] {
                let text = draw(&app, width, height);
                assert!(text.contains("? help | q leave"));
                let line = footer(&app, width - 17).to_string();
                assert!(line.len() < usize::from(width - 17));
                assert!(!line.ends_with(" |"));
                if focus == Focus::Projects && width == 60 {
                    assert!(line.contains("a add"));
                }
            }
        }
        app.logs.begin_selection(20);
        assert_eq!(footer(&app, 19).to_string(), "y copy | Esc");
    }

    #[test]
    fn themes_keep_content_readable_and_fallbacks_preserve_text_and_selection() {
        use super::super::appearance::Theme;
        fn luminance(color: Color) -> f64 {
            let Color::Rgb(r, g, b) = color else {
                panic!("Expected resolved color")
            };
            let channel = |v: u8| {
                let n = f64::from(v) / 255.0;
                if n <= 0.04045 {
                    n / 12.92
                } else {
                    ((n + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
        }
        let mut app = searched_app();
        app.logs.begin_selection(20);
        let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
        for theme in [Theme::Dark, Theme::Light] {
            app.appearance.theme = theme;
            for help in [false, true] {
                app.help = help;
                terminal.draw(|frame| render(frame, &app)).unwrap();
                for cell in &terminal.backend().buffer().content {
                    if cell.symbol().trim().is_empty() {
                        continue;
                    }
                    let (fg, bg) = (luminance(cell.fg), luminance(cell.bg));
                    let ratio = (fg.max(bg) + 0.05) / (fg.min(bg) + 0.05);
                    assert!(ratio >= 4.5, "{theme:?} {:?}: {ratio}", cell);
                }
            }
        }
        app.help = false;
        app.appearance.theme = Theme::Terminal;
        assert!(draw(&app, 120, 30).contains("needle"));
        app.appearance.no_color = true;
        app.appearance.ascii = true;
        app.appearance.reduced_motion = true;
        let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        for cell in &terminal.backend().buffer().content {
            assert!(
                cell.symbol().trim().is_empty()
                    || (cell.fg == Color::Reset && cell.bg == Color::Reset),
                "{cell:?}"
            );
        }
        assert!(
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .any(|cell| cell.modifier.contains(Modifier::UNDERLINED))
        );
        assert_eq!(app.logs.selected_text().unwrap().0, "前 needle 中 needle\n");
        app.logs.selection = None;
        app.focus(Focus::Projects);
        let text = draw(&app, 120, 30);
        assert!(text.contains("> web") && text.contains("- Stopped"));
    }
}
