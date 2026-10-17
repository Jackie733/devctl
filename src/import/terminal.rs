use std::io::{IsTerminal, Write};

use crossterm::{
    cursor::{Hide, Show},
    event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use futures_util::StreamExt;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    style::{Color, Style},
    text::Line,
    widgets::Paragraph,
};

use crate::discovery::safe;

pub(super) fn available() -> bool {
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

struct Raw {
    alternate: bool,
}
impl Raw {
    fn enter(alternate: bool) -> Result<Self, String> {
        enable_raw_mode().map_err(|error| error.to_string())?;
        let guard = Self { alternate };
        if alternate {
            execute!(
                std::io::stdout(),
                EnterAlternateScreen,
                Hide,
                Clear(ClearType::All)
            )
            .map_err(|error| error.to_string())?;
        }
        Ok(guard)
    }
}
impl Drop for Raw {
    fn drop(&mut self) {
        if self.alternate {
            let _ = execute!(std::io::stdout(), LeaveAlternateScreen, Show);
        }
        let _ = disable_raw_mode();
    }
}

/// Disabled choices remain visible, such as library packages without scripts.
pub(super) async fn choose(
    title: &str,
    labels: &[String],
    enabled: &[bool],
    multiple: bool,
    initial: &[usize],
) -> Result<Vec<usize>, String> {
    let _raw = Raw::enter(true)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))
        .map_err(|error| error.to_string())?;
    let mut current = initial.first().copied().unwrap_or(0);
    let mut selected = vec![false; labels.len()];
    for &index in initial {
        selected[index] = true;
    }
    let mut events = EventStream::new();
    loop {
        let mut usable = false;
        terminal
            .draw(|frame| {
                let area = frame.area();
                usable = area.width >= 50 && area.height >= 10;
                if !usable {
                    frame.render_widget(
                        Paragraph::new("Resize to at least 50 x 10. Esc cancels import."),
                        area,
                    );
                    return;
                }
                let rows = usize::from(area.height.saturating_sub(6));
                let first = current.saturating_sub(rows.saturating_sub(1));
                let mut lines = vec![
                    Line::from(safe(title)),
                    Line::from(
                        "j/k or arrows: move | Space: select | Enter: continue | Esc: cancel",
                    ),
                    Line::from("No project scripts will run. Root/member commands may overlap."),
                    Line::default(),
                ];
                for index in first..labels.len().min(first + rows) {
                    let marker = if !enabled[index] {
                        "[-]"
                    } else if multiple && selected[index] {
                        "[x]"
                    } else {
                        "[ ]"
                    };
                    let text = format!(
                        "{} {marker} {}",
                        if current == index { ">" } else { " " },
                        safe(&labels[index])
                    );
                    lines.push(Line::styled(
                        text,
                        if current == index {
                            Style::default().fg(Color::Black).bg(Color::Cyan)
                        } else {
                            Style::default()
                        },
                    ));
                }
                frame.render_widget(Paragraph::new(lines), area);
            })
            .map_err(|error| error.to_string())?;
        let Some(event) = events.next().await else {
            return Err("Import selection cancelled.".into());
        };
        let event = event.map_err(|error| error.to_string())?;
        if let Event::Key(key) = event
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Esc => return Err("Import selection cancelled; no files changed.".into()),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Err("Import selection cancelled; no files changed.".into());
                }
                KeyCode::Down | KeyCode::Char('j') if usable => {
                    current = (current + 1).min(labels.len().saturating_sub(1))
                }
                KeyCode::Up | KeyCode::Char('k') if usable => current = current.saturating_sub(1),
                KeyCode::Char(' ') if usable && multiple && enabled[current] => {
                    selected[current] = !selected[current]
                }
                KeyCode::Enter if usable => {
                    let result = if multiple {
                        selected
                            .iter()
                            .enumerate()
                            .filter_map(|(index, selected)| selected.then_some(index))
                            .collect::<Vec<_>>()
                    } else if enabled[current] {
                        vec![current]
                    } else {
                        Vec::new()
                    };
                    if !result.is_empty() {
                        return Ok(result);
                    }
                }
                _ => {}
            }
        }
    }
}

pub(super) async fn exact(prompt: &str) -> Result<String, String> {
    let _raw = Raw::enter(false)?;
    print!("{prompt} (Esc cancels): ");
    std::io::stdout()
        .flush()
        .map_err(|error| error.to_string())?;
    let mut events = EventStream::new();
    let mut text = String::new();
    while let Some(event) = events.next().await {
        if let Event::Key(key) = event.map_err(|error| error.to_string())?
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Esc => break,
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                KeyCode::Enter if !text.is_empty() => {
                    print!("\r\n");
                    return Ok(text);
                }
                KeyCode::Backspace if !text.is_empty() => {
                    text.pop();
                    print!("\x08 \x08");
                }
                KeyCode::Char(character) if character.is_ascii_graphic() && text.len() < 128 => {
                    text.push(character);
                    print!("{character}");
                }
                _ => {}
            }
            std::io::stdout()
                .flush()
                .map_err(|error| error.to_string())?;
        }
    }
    Err("Import selection cancelled; no files changed.".into())
}
