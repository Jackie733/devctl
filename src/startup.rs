use std::{fs::OpenOptions, io::Write};

pub fn detach() -> Result<(), String> {
    nix::unistd::setsid().map_err(|error| format!("Failed to detach supervisor: {error}"))?;
    let null = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/null")
        .map_err(|error| format!("Failed to open /dev/null: {error}"))?;
    nix::unistd::dup2_stdin(&null).map_err(|error| format!("Failed to redirect stdin: {error}"))?;
    nix::unistd::dup2_stderr(&null)
        .map_err(|error| format!("Failed to redirect stderr: {error}"))?;
    Ok(())
}

pub fn ready() -> Result<(), String> {
    // This pipe is private to the launcher. Closing it cannot stop a workspace.
    let mut output = std::io::stdout().lock();
    let _ = output.write_all(b"READY\n");
    let _ = output.flush();
    let null = OpenOptions::new()
        .write(true)
        .open("/dev/null")
        .map_err(|error| format!("Failed to open /dev/null: {error}"))?;
    nix::unistd::dup2_stdout(&null).map_err(|error| format!("Failed to redirect stdout: {error}"))
}

pub fn failed(error: &str) {
    let message: String = error.chars().take(512).collect();
    let _ = writeln!(std::io::stdout().lock(), "ERROR\n{message}");
}

pub fn locked() {
    let _ = std::io::stdout().lock().write_all(b"LOCKED\n");
}
