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

/// Run before constructing Tokio, while only inherited non-stdio descriptors exist.
/// A detached supervisor must not keep a caller's pipes or terminal handles open.
#[cfg(target_os = "macos")]
pub fn close_inherited_descriptors() -> Result<(), String> {
    use nix::{errno::Errno, libc, unistd::close};

    let mut fds = vec![
        libc::proc_fdinfo {
            proc_fd: 0,
            proc_fdtype: 0
        };
        64
    ];
    loop {
        let size = (fds.len() * std::mem::size_of::<libc::proc_fdinfo>()) as i32;
        // SAFETY: fds is writable for size bytes. This runs before runtime/file
        // creation and reads only the current process's descriptor table.
        let bytes = unsafe {
            libc::proc_pidinfo(
                libc::getpid(),
                libc::PROC_PIDLISTFDS,
                0,
                fds.as_mut_ptr().cast(),
                size,
            )
        };
        if bytes <= 0 || bytes % std::mem::size_of::<libc::proc_fdinfo>() as i32 != 0 {
            return Err(format!(
                "Failed to inspect inherited descriptors: {}",
                Errno::last()
            ));
        }
        if bytes < size {
            fds.truncate(bytes as usize / std::mem::size_of::<libc::proc_fdinfo>());
            break;
        }
        if fds.len() >= 65536 {
            return Err(
                "Inherited descriptor count exceeds the 65536-entry inspection limit.".into(),
            );
        }
        fds.resize(
            fds.len() * 2,
            libc::proc_fdinfo {
                proc_fd: 0,
                proc_fdtype: 0,
            },
        );
    }
    for fd in fds.into_iter().filter(|fd| fd.proc_fd > 2) {
        match close(fd.proc_fd) {
            Ok(()) | Err(Errno::EBADF) => {}
            Err(error) => return Err(format!("Failed to close inherited descriptor: {error}")),
        }
    }
    Ok(())
}
