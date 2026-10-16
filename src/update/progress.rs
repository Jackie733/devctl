use std::{
    io::{self, IsTerminal, Write},
    process::ExitStatus,
    time::{Duration, Instant},
};

use crossterm::{
    cursor::MoveToColumn,
    queue,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{Clear, ClearType},
};
use tokio::{io::AsyncReadExt, process::Child, time};

const FRAMES: [char; 8] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧'];
const REFRESH: Duration = Duration::from_millis(100);
const MIB: f64 = 1024.0 * 1024.0;

/// Owns only a transient stderr line. Never changes terminal modes or update state.
pub(super) struct Progress {
    interactive: bool,
    color: bool,
    visible: bool,
    paused: bool,
    partial_diagnostic: bool,
    stage: Option<&'static str>,
    total: Option<u64>,
    received: u64,
    started: Instant,
    frame: usize,
}

impl Progress {
    pub fn new() -> Self {
        Self {
            interactive: io::stderr().is_terminal()
                && io::stdout().is_terminal()
                && std::env::var_os("TERM").is_none_or(|value| value != "dumb"),
            color: std::env::var_os("NO_COLOR").is_none(),
            visible: false,
            paused: false,
            partial_diagnostic: false,
            stage: None,
            total: None,
            received: 0,
            started: Instant::now(),
            frame: 0,
        }
    }

    pub fn begin(&mut self, stage: &'static str, total: Option<u64>) {
        self.clear();
        self.stage = Some(stage);
        self.total = total;
        self.received = 0;
        self.started = Instant::now();
        self.frame = 0;
        self.paused = false;
        if self.interactive {
            self.tick();
        } else {
            let _ = writeln!(io::stderr(), "{stage}...");
        }
    }

    pub fn received(&mut self, bytes: u64) {
        self.received = bytes;
    }

    pub fn tick(&mut self) {
        let Some(stage) = self.stage else { return };
        if !self.interactive || self.paused {
            return;
        }
        let columns = crossterm::terminal::size().map_or(80, |(columns, _)| columns as usize);
        let line = line(
            stage,
            self.total,
            self.received,
            self.started.elapsed(),
            columns,
            self.frame,
        );
        self.frame = (self.frame + 1) % FRAMES.len();
        let mut stderr = io::stderr().lock();
        let result = (|| -> io::Result<()> {
            queue!(stderr, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
            if self.color {
                queue!(stderr, SetForegroundColor(Color::Cyan))?;
            }
            queue!(stderr, Print(line))?;
            if self.color {
                queue!(stderr, ResetColor)?;
            }
            stderr.flush()
        })();
        // Presentation errors must not turn a successful installation into a failure.
        self.visible = result.is_ok();
        if result.is_err() {
            if self.color {
                let _ = queue!(stderr, ResetColor);
                let _ = stderr.flush();
            }
            self.interactive = false;
        }
    }

    pub fn complete(&mut self, message: &str) {
        self.clear();
        self.stage = None;
        self.status(message, true);
        self.partial_diagnostic = false;
    }

    pub fn fail(&mut self) {
        self.clear();
        if let Some(stage) = self.stage.take() {
            self.status(&format!("{stage} failed"), false);
            self.partial_diagnostic = false;
        }
    }

    fn status(&self, message: &str, success: bool) {
        let mut stderr = io::stderr().lock();
        if self.partial_diagnostic {
            let _ = writeln!(stderr);
        }
        if self.interactive {
            if self.color {
                let _ = queue!(
                    stderr,
                    SetForegroundColor(if success { Color::Green } else { Color::Red })
                );
            }
            let _ = write!(stderr, "{} ", if success { '✓' } else { '✗' });
            if self.color {
                let _ = queue!(stderr, ResetColor);
            }
        }
        let _ = writeln!(stderr, "{message}");
    }

    fn clear(&mut self) {
        if self.visible {
            let mut stderr = io::stderr().lock();
            let _ = queue!(stderr, MoveToColumn(0), Clear(ClearType::CurrentLine));
            let _ = stderr.flush();
            self.visible = false;
        }
    }

    /// Suspend animation once a child writes, so even partial diagnostics stay visible.
    pub fn relay(&mut self, bytes: &[u8], stderr: bool) {
        self.clear();
        self.paused = true;
        if stderr {
            self.partial_diagnostic = !bytes.ends_with(b"\n");
            let _ = io::stderr().write_all(bytes);
        } else {
            let _ = io::stdout().write_all(bytes);
        }
    }

    /// Stream installer output without buffering it or letting inherited pipes hang exit.
    pub async fn wait(&mut self, child: &mut Child) -> Result<ExitStatus, String> {
        let mut stdout = child.stdout.take().ok_or("Missing installer output.")?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or("Missing installer diagnostics.")?;
        let mut out = [0u8; 8192];
        let mut err = [0u8; 8192];
        let (mut out_open, mut err_open) = (true, true);
        let mut status = None;
        let mut drain_deadline = time::Instant::now();
        let mut refresh = interval();
        loop {
            if status.is_some() && !out_open && !err_open {
                break;
            }
            tokio::select! {
                result = child.wait(), if status.is_none() => {
                    status = Some(result.map_err(|error| format!("Cannot wait for installer: {error}"))?);
                    drain_deadline = time::Instant::now() + Duration::from_secs(1);
                }
                result = stdout.read(&mut out), if out_open => {
                    match result {
                        Ok(0) => out_open = false,
                        Err(error) => {
                            out_open = false;
                            self.relay(format!("Cannot read installer output: {error}\n").as_bytes(), true);
                        }
                        Ok(count) => self.relay(&out[..count], false),
                    }
                }
                result = stderr.read(&mut err), if err_open => {
                    match result {
                        Ok(0) => err_open = false,
                        Err(error) => {
                            err_open = false;
                            self.relay(format!("Cannot read installer diagnostics: {error}\n").as_bytes(), true);
                        }
                        Ok(count) => self.relay(&err[..count], true),
                    }
                }
                _ = refresh.tick() => self.tick(),
                _ = time::sleep_until(drain_deadline), if status.is_some() => {
                    self.relay(b"Installer output did not close after exit; remaining output omitted.\n", true);
                    break;
                }
            }
        }
        status.ok_or_else(|| "Missing installer exit status.".into())
    }
}

impl Drop for Progress {
    fn drop(&mut self) {
        self.clear();
    }
}

pub(super) fn interval() -> time::Interval {
    let mut interval = time::interval_at(time::Instant::now() + REFRESH, REFRESH);
    interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
    interval
}

fn line(
    stage: &str,
    total: Option<u64>,
    received: u64,
    elapsed: Duration,
    columns: usize,
    frame: usize,
) -> String {
    let prefix = format!("{} {stage}", FRAMES[frame % FRAMES.len()]);
    let Some(total) = total.filter(|total| *total > 0) else {
        return prefix.chars().take(columns.saturating_sub(1)).collect();
    };
    let received = received.min(total);
    let percent = received * 100 / total;
    if columns < 30 {
        return format!("{} {percent:>3}% {stage}", FRAMES[frame % FRAMES.len()])
            .chars()
            .take(columns.saturating_sub(1))
            .collect();
    }
    let mut details = format!("{percent:>3}%");
    if columns >= 50 {
        details.push_str(&format!(
            "  {:.1}/{:.1} MiB",
            received as f64 / MIB,
            total as f64 / MIB
        ));
    }
    if columns >= 90 {
        if elapsed >= Duration::from_millis(500) && received > 0 {
            let speed = received as f64 / elapsed.as_secs_f64();
            let remaining = ((total - received) as f64 / speed).ceil() as u64;
            details.push_str(&format!("  {:.1} MiB/s  ETA {remaining}s", speed / MIB));
        } else {
            details.push_str("  -- MiB/s  ETA --");
        }
    }
    let available = columns.saturating_sub(prefix.chars().count() + details.chars().count() + 5);
    let bar = available.min(24);
    let text = if bar >= 6 {
        let filled = bar as u64 * received / total;
        format!(
            "{prefix}  {}{}  {details}",
            "━".repeat(filled as usize),
            "─".repeat(bar - filled as usize)
        )
    } else {
        format!("{prefix}  {details}")
    };
    text.chars().take(columns.saturating_sub(1)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn an_open_output_pipe_cannot_hold_up_an_exited_installer() {
        use std::process::Stdio;
        use tokio::process::Command;

        // Model a descendant retaining an output pipe, while keeping its owner
        // directly under the test so no orphan process is left behind.
        let mut holder = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut child = Command::new("true")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        child.stdout = holder.stdout.take();
        let result = time::timeout(Duration::from_secs(3), Progress::new().wait(&mut child)).await;
        holder.kill().await.unwrap();
        assert!(result.unwrap().unwrap().success());
    }

    #[test]
    fn download_information_uses_real_bytes_and_elapsed_time() {
        let text = line(
            "Downloading",
            Some(4 * 1024 * 1024),
            1024 * 1024,
            Duration::from_secs(2),
            120,
            0,
        );
        assert!(text.contains("25%"), "{text}");
        assert!(text.contains("1.0/4.0 MiB"), "{text}");
        assert!(text.contains("0.5 MiB/s"), "{text}");
        assert!(text.contains("ETA 6s"), "{text}");
        let text = line("Downloading", Some(100), 0, Duration::from_secs(2), 120, 0);
        assert!(text.contains("0%") && text.contains("ETA --"), "{text}");
    }

    #[test]
    fn frames_fit_narrow_terminals_and_unknown_totals_do_not_invent_percentages() {
        for width in [0, 1, 15, 30, 50, 80, 100] {
            let text = line(
                "Downloading",
                Some(4 * 1024 * 1024),
                1024 * 1024,
                Duration::from_secs(1),
                width,
                3,
            );
            assert!(text.chars().count() <= width.saturating_sub(1));
        }
        let text = line("Checking release", None, 0, Duration::from_secs(10), 80, 0);
        assert!(!text.contains('%'));
        assert_ne!(
            text,
            line("Checking release", None, 0, Duration::from_secs(10), 80, 1)
        );
        let text = line("Downloading", Some(100), 200, Duration::from_secs(1), 80, 0);
        assert!(text.contains("100%"));
    }
}
