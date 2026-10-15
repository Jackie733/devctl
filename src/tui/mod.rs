mod terminal;

use std::io::{self, stdout};

use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use futures_util::StreamExt;
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    widgets::{Block, Paragraph},
};
use tokio::signal::unix::{SignalKind, signal};

pub async fn run() -> Result<(), String> {
    let mut interrupt = signal(SignalKind::interrupt()).map_err(|error| error.to_string())?;
    let mut terminate = signal(SignalKind::terminate()).map_err(|error| error.to_string())?;
    let mut hangup = signal(SignalKind::hangup()).map_err(|error| error.to_string())?;
    let mut guard = terminal::TerminalGuard::enter().map_err(|error| error.to_string())?;

    let result: io::Result<()> = async {
        let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
        let mut events = EventStream::new();
        terminal.draw(render)?;

        loop {
            tokio::select! {
                event = events.next() => {
                    let event = event.ok_or_else(|| {
                        io::Error::new(io::ErrorKind::UnexpectedEof, "Terminal event stream ended.")
                    })??;
                    match event {
                        Event::Key(key) if key.kind == KeyEventKind::Press => {
                            let quit = key.code == KeyCode::Char('q')
                                && key.modifiers.is_empty();
                            let interrupt = key.code == KeyCode::Char('c')
                                && key.modifiers.contains(KeyModifiers::CONTROL);
                            if quit || interrupt {
                                return Ok(());
                            }
                        }
                        Event::Resize(_, _) => {
                            terminal.draw(render)?;
                        }
                        _ => {}
                    }
                }
                _ = interrupt.recv() => return Ok(()),
                _ = terminate.recv() => return Ok(()),
                _ = hangup.recv() => return Ok(()),
            }
        }
    }
    .await;

    match (result, guard.restore()) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(format!("TUI failed: {error}")),
        (Ok(()), Err(error)) => Err(format!("Terminal restore failed: {error}")),
        (Err(error), Err(cleanup)) => Err(format!(
            "TUI failed: {error}; terminal restore failed: {cleanup}"
        )),
    }
}

fn render(frame: &mut Frame) {
    let area = frame.area();
    if area.width < 40 || area.height < 6 {
        frame.render_widget(
            Paragraph::new("devctl\nNot connected\nq / Ctrl-C: leave"),
            area,
        );
        return;
    }
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);
    let [projects, logs] =
        Layout::horizontal([Constraint::Length(24), Constraint::Min(0)]).areas(body);
    frame.render_widget(Paragraph::new("devctl | Not connected"), header);
    frame.render_widget(
        Paragraph::new("No snapshot yet.").block(Block::bordered().title("Projects")),
        projects,
    );
    frame.render_widget(
        Paragraph::new("Not connected to the workspace.").block(Block::bordered().title("Logs")),
        logs,
    );
    frame.render_widget(Paragraph::new("q / Ctrl-C: leave"), footer);
}
