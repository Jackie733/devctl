use std::{
    io::{self, IsTerminal, Write, stdout},
    process::Stdio,
    time::Duration,
};

use crossterm::{
    clipboard::CopyToClipboard,
    cursor::{Hide, Show},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use tokio::{io::AsyncWriteExt, process::Command, time::timeout};

#[derive(Default)]
pub struct TerminalGuard {
    raw_mode: bool,
    alternate_screen: bool,
    cursor_hidden: bool,
}

impl TerminalGuard {
    pub fn enter() -> io::Result<Self> {
        if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
            return Err(io::Error::other(
                "TUI requires a terminal on stdin and stdout.",
            ));
        }

        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = disable_raw_mode();
            let _ = execute!(stdout(), LeaveAlternateScreen);
            let _ = execute!(stdout(), Show);
            previous_hook(info);
        }));

        let mut guard = Self::default();
        let result = (|| {
            guard.raw_mode = true;
            enable_raw_mode()?;
            guard.alternate_screen = true;
            execute!(stdout(), EnterAlternateScreen)?;
            guard.cursor_hidden = true;
            execute!(stdout(), Hide)?;
            Ok(())
        })();

        if let Err(error) = result {
            return match guard.restore() {
                Ok(()) => Err(error),
                Err(cleanup) => Err(io::Error::other(format!(
                    "{error}; restore failed: {cleanup}"
                ))),
            };
        }
        Ok(guard)
    }

    pub fn restore(&mut self) -> io::Result<()> {
        let mut errors = Vec::new();
        if self.raw_mode {
            match disable_raw_mode() {
                Ok(()) => self.raw_mode = false,
                Err(error) => errors.push(format!("raw mode: {error}")),
            }
        }
        if self.alternate_screen {
            match execute!(stdout(), LeaveAlternateScreen) {
                Ok(()) => self.alternate_screen = false,
                Err(error) => errors.push(format!("alternate screen: {error}")),
            }
        }
        if self.cursor_hidden {
            match execute!(stdout(), Show) {
                Ok(()) => self.cursor_hidden = false,
                Err(error) => errors.push(format!("cursor: {error}")),
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(io::Error::other(errors.join("; ")))
        }
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

pub enum CopyTarget {
    Windows,
    Terminal,
}

pub async fn copy_logs(text: &str) -> Result<CopyTarget, String> {
    let wsl = std::env::var_os("WSL_DISTRO_NAME").is_some()
        || std::fs::read_to_string("/proc/sys/kernel/osrelease")
            .is_ok_and(|release| release.to_ascii_lowercase().contains("microsoft"));
    // Over SSH, target the displaying terminal rather than a remote Windows desktop.
    if wsl && std::env::var_os("SSH_CONNECTION").is_none() {
        copy_to_command(Command::new("clip.exe"), text, Duration::from_secs(3)).await?;
        Ok(CopyTarget::Windows)
    } else {
        copy_osc52(&mut stdout().lock(), text).map_err(|error| error.to_string())?;
        Ok(CopyTarget::Terminal)
    }
}

async fn copy_to_command(
    mut command: Command,
    text: &str,
    deadline: Duration,
) -> Result<(), String> {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("cannot start clip.exe: {error}"))?;
    let mut input = child.stdin.take().expect("Clipboard stdin is piped.");
    // clip.exe expects Windows Unicode input; UTF-16LE avoids code-page loss.
    let bytes: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let result = timeout(deadline, async {
        input
            .write_all(&bytes)
            .await
            .map_err(|error| error.to_string())?;
        drop(input);
        let status = child.wait().await.map_err(|error| error.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("clip.exe exited with {status}"))
        }
    })
    .await
    .unwrap_or_else(|_| Err("clip.exe timed out".into()));
    if result.is_err() {
        // Kill and reap a stalled reader as well as a child whose pipe closed early.
        let _ = child.kill().await;
    }
    result
}

// A successful OSC 52 write confirms transmission, not clipboard acceptance.
fn copy_osc52(writer: &mut impl Write, text: &str) -> io::Result<()> {
    execute!(writer, CopyToClipboard::to_clipboard_from(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copying_encodes_text_as_a_clipboard_command_and_reports_write_failure() {
        let mut output = Vec::new();
        copy_osc52(&mut output, "hello\n").unwrap();
        assert_eq!(output, b"\x1b]52;c;aGVsbG8K\x1b\\");
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("closed terminal"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        assert!(
            copy_osc52(&mut Broken, "hello")
                .unwrap_err()
                .to_string()
                .contains("closed terminal")
        );
    }

    #[tokio::test]
    async fn native_clipboard_receives_unicode_and_reports_exit_failure_and_timeout() {
        let mut command = Command::new("python3");
        command.args([
            "-c",
            "import sys; assert sys.stdin.buffer.read().decode('utf-16le') == '中文🙂\\tline\\n'",
        ]);
        copy_to_command(command, "中文🙂\tline\n", Duration::from_secs(3))
            .await
            .unwrap();
        let mut command = Command::new("sh");
        command.args(["-c", "exit 7"]);
        assert!(
            copy_to_command(command, "", Duration::from_secs(3))
                .await
                .unwrap_err()
                .contains("exit status: 7")
        );
        let mut command = Command::new("sh");
        command.args(["-c", "exec sleep 60"]);
        assert!(
            copy_to_command(command, "test", Duration::from_millis(50))
                .await
                .unwrap_err()
                .contains("timed out")
        );
    }
}
