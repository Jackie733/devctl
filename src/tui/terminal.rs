use std::io::{self, IsTerminal, Write, stdout};

use crossterm::{
    clipboard::CopyToClipboard,
    cursor::{Hide, Show},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};

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

// OSC 52 targets the terminal host clipboard, including a local host over SSH.
// A successful write only confirms delivery to the terminal, not clipboard acceptance.
pub fn copy_logs(writer: &mut impl Write, text: &str) -> io::Result<()> {
    execute!(writer, CopyToClipboard::to_clipboard_from(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copying_encodes_text_as_a_clipboard_command_and_reports_write_failure() {
        let mut output = Vec::new();
        copy_logs(&mut output, "hello\n").unwrap();
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
            copy_logs(&mut Broken, "hello")
                .unwrap_err()
                .to_string()
                .contains("closed terminal")
        );
    }
}
