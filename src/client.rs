use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
    process::Command,
    time::{Instant, sleep, timeout, timeout_at},
};

use crate::{
    config,
    control::{self, Request, Response},
    manager::Manager,
    run_lock::RunLock,
};

enum ConnectError {
    Missing(String),
    Other(String),
}
impl ConnectError {
    fn message(self) -> String {
        match self {
            Self::Missing(error) | Self::Other(error) => error,
        }
    }
}

pub fn socket_path(config: &Path) -> PathBuf {
    let mut name = config.as_os_str().to_os_string();
    name.push(".devctl");
    PathBuf::from(name).join("control.sock")
}

async fn connect(config: &Path) -> Result<UnixStream, ConnectError> {
    timeout(Duration::from_secs(2), connect_inner(config))
        .await
        .map_err(|_| ConnectError::Other("Workspace connection timed out.".into()))?
}

async fn connect_inner(config: &Path) -> Result<UnixStream, ConnectError> {
    let path = socket_path(config);
    let mut stream = UnixStream::connect(&path).await.map_err(|error| {
        let missing = matches!(
            error.kind(),
            std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
        );
        let error = format!("Cannot connect to workspace {}: {error}", path.display());
        if missing {
            ConnectError::Missing(error)
        } else {
            ConnectError::Other(error)
        }
    })?;
    timeout(Duration::from_secs(2), async {
        stream.write_all(control::CONTROL_HEADER).await?;
        control::verify_identity(&mut stream, config).await
    })
    .await
    .map_err(|_| ConnectError::Other("Workspace identity handshake timed out.".into()))?
    .map_err(|error| {
        ConnectError::Other(format!("Workspace identity handshake failed: {error}"))
    })?;
    Ok(stream)
}

fn preflight(config_path: &Path, request: &Request) -> Result<(), String> {
    let config = config::load(config_path)?;
    let manager = Manager::new(config.services);
    if let Request::Start { targets, all } = request {
        let selected = if *all {
            manager.default_targets()
        } else {
            targets.clone()
        };
        manager.check_start(&selected)?;
    }
    Ok(())
}

async fn launch(config: &Path) -> Result<UnixStream, String> {
    let deadline = Instant::now() + Duration::from_secs(8);
    let executable =
        std::env::current_exe().map_err(|error| format!("Cannot locate devctl: {error}"))?;
    let mut child = Command::new(executable)
        .arg("--config")
        .arg(config)
        .args(["supervise", "--detached"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("Failed to launch supervisor: {error}"))?;
    let mut report = Vec::new();
    let stdout = child.stdout.take().expect("Piped startup report exists.");
    timeout_at(deadline, stdout.take(4097).read_to_end(&mut report))
        .await
        .map_err(|_| "Supervisor startup timed out; query status before retrying.".to_owned())?
        .map_err(|error| format!("Failed to read startup report: {error}"))?;
    if report.len() > 4096 {
        return Err("Supervisor startup report exceeds size limit.".into());
    }
    let report = String::from_utf8_lossy(&report);
    let ready = report == "READY\n";
    if !ready && !report.starts_with("LOCKED\n") {
        let _ = timeout(Duration::from_secs(1), child.wait()).await;
        return Err(format!("Supervisor startup failed: {}", report.trim()));
    }
    let mut last_error = String::new();
    while Instant::now() < deadline {
        match timeout_at(deadline, connect(config))
            .await
            .map_err(|_| "Supervisor readiness probe timed out.".to_owned())?
        {
            Ok(stream) => return Ok(stream),
            Err(ConnectError::Other(error)) => return Err(error),
            Err(ConnectError::Missing(error)) => last_error = error,
        }
        let _ = child.try_wait();
        sleep(Duration::from_millis(20)).await;
    }
    Err(format!(
        "Workspace did not become available: {last_error}. {}",
        report.trim()
    ))
}

pub async fn execute(path: &Path, request: &Request, create: bool) -> Result<Response, String> {
    let config = path
        .canonicalize()
        .map_err(|error| format!("Failed to resolve config path {}: {error}", path.display()))?;
    let mut stream = match connect(&config).await {
        Ok(stream) => stream,
        Err(ConnectError::Missing(_)) if create => {
            preflight(&config, request)?;
            launch(&config).await?
        }
        Err(error) => return Err(error.message()),
    };
    let response: Response = timeout(Duration::from_secs(15), async {
        control::write_frame(&mut stream, request, control::MAX_REQUEST).await?;
        control::read_frame(&mut stream, control::MAX_RESPONSE).await
    })
    .await
    .map_err(|_| {
        "Request timed out; completion is unknown. Query status before retrying.".to_owned()
    })?
    .map_err(|error| format!("Control connection failed; completion is unknown: {error}"))?;
    if let Some(error) = &response.error {
        return Err(error.clone());
    }
    if matches!(request, Request::Shutdown {}) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match RunLock::try_acquire(&config) {
                Ok(Some(guard)) => {
                    drop(guard);
                    break;
                }
                Ok(None) if Instant::now() < deadline => {
                    sleep(Duration::from_millis(20)).await;
                }
                Ok(None) => {
                    return Err("Cleanup completed but workspace lock is still held.".into());
                }
                Err(error) => {
                    return Err(format!(
                        "Cleanup completed but workspace release is unconfirmed: {error}"
                    ));
                }
            }
        }
    }
    Ok(response)
}
