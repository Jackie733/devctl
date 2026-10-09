use std::process::{Command, ExitStatus, Stdio};

use crate::config::Service;

pub fn run(id: &str, service: &Service) -> Result<ExitStatus, String> {
    let (program, args) = service
        .cmd
        .split_first()
        .ok_or_else(|| format!("Service '{id}' has no command."))?;

    let cwd = service
        .cwd
        .as_ref()
        .ok_or_else(|| format!("Service '{id}' has no resolved working directory."))?;

    let mut child = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("Failed to start service '{id}': {error}"))?;

    println!("Started service '{id}' with PID {}.", child.id());

    let status = child
        .wait()
        .map_err(|error| format!("Failed to wait for service '{id}': {error}"))?;

    println!("Service '{id}' exited: {status}");

    Ok(status)
}
