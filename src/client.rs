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

#[derive(Debug)]
pub enum ClientError {
    Preparation(String),
    Connection(String),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Preparation(message) | Self::Connection(message) => formatter.write_str(message),
        }
    }
}

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
    let response = transact(path, request, create)
        .await
        .map_err(|error| error.to_string())?;
    if let Some(error) = &response.error {
        return Err(error.clone());
    }
    Ok(response)
}

/// Read without creating a supervisor. The run lock, not a failed socket probe,
/// proves that offline reading cannot race a live archive writer or cleanup.
pub async fn read_logs(path: &Path, request: Request) -> Result<Response, String> {
    let config = path.canonicalize().map_err(|e| e.to_string())?;
    // A cold read with no archive must not create even a run-lock sidecar.
    let archived = crate::archive::directory(&config).is_ok_and(|dir| dir.exists());
    if !archived {
        return execute(&config, &request, false).await;
    }
    if let Some(_guard) = RunLock::try_acquire(&config)? {
        let settings = config::load_log_settings(&config)?;
        return match request {
            Request::ReadLogs { query } => {
                crate::archive::offline(&config, &settings, &query).map(|logs| Response {
                    logs: Some(logs),
                    ..Response::default()
                })
            }
            Request::LogSessions {} => crate::archive::directory(&config)
                .and_then(|dir| crate::archive::sessions(&dir, &config, &settings))
                .map(|sessions| Response {
                    sessions,
                    ..Response::default()
                }),
            _ => Err("Invalid offline log request.".into()),
        };
    }
    execute(&config, &request, false).await
}

/// Preserve server feedback and workspace identity for interactive clients.
pub async fn transact(
    path: &Path,
    request: &Request,
    create: bool,
) -> Result<Response, ClientError> {
    use ClientError::{Connection, Preparation};

    let config = path.canonicalize().map_err(|error| {
        Preparation(format!(
            "Failed to resolve config path {}: {error}",
            path.display()
        ))
    })?;
    let mut stream = match connect(&config).await {
        Ok(stream) => stream,
        Err(ConnectError::Missing(_)) if create => {
            preflight(&config, request).map_err(Preparation)?;
            launch(&config).await.map_err(Connection)?
        }
        Err(error) => return Err(Connection(error.message())),
    };
    let response: Response = timeout(Duration::from_secs(15), async {
        control::write_frame(&mut stream, request, control::MAX_REQUEST).await?;
        control::read_frame(&mut stream, control::MAX_RESPONSE).await
    })
    .await
    .map_err(|_| {
        Connection(
            "Request timed out; completion is unknown. Query status before retrying.".to_owned(),
        )
    })?
    .map_err(|error| {
        Connection(format!(
            "Control connection failed; completion is unknown: {error}"
        ))
    })?;
    if response.workspace_id.is_empty() {
        return Err(Connection(
            "Control response is missing its workspace identity.".into(),
        ));
    }
    if response.error.is_none() && matches!(request, Request::Shutdown {}) {
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
                    return Err(Connection(
                        "Cleanup completed but workspace lock is still held.".into(),
                    ));
                }
                Err(error) => {
                    return Err(Connection(format!(
                        "Cleanup completed but workspace release is unconfirmed: {error}"
                    )));
                }
            }
        }
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };
    use tokio::net::UnixListener;

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Endpoint(PathBuf);
    impl Endpoint {
        fn new() -> (Self, PathBuf, UnixListener) {
            let temporary = if cfg!(target_os = "macos") {
                PathBuf::from("/tmp")
            } else {
                std::env::temp_dir()
            };
            let root = temporary.join(format!(
                "devctl-client-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            let config = root.join("config.toml");
            fs::write(&config, "Existing instances do not reload configuration.").unwrap();
            let config = config.canonicalize().unwrap();
            let socket = socket_path(&config);
            fs::create_dir(socket.parent().unwrap()).unwrap();
            let listener = UnixListener::bind(socket).unwrap();
            (Self(root), config, listener)
        }
    }
    impl Drop for Endpoint {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[tokio::test]
    async fn transaction_preserves_server_error_identity_and_cli_wrapper_preserves_error_text() {
        let (_endpoint, config, listener) = Endpoint::new();
        let identity = config.clone();
        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut header = [0; 9];
                stream.read_exact(&mut header).await.unwrap();
                assert_eq!(&header, control::CONTROL_HEADER);
                crate::probe::write_identity(&mut stream, &identity)
                    .await
                    .unwrap();
                let _: Request = control::read_frame(&mut stream, control::MAX_REQUEST)
                    .await
                    .unwrap();
                let response = Response {
                    workspace_id: "one".into(),
                    ..Response::failed("Completion unknown; query status.")
                };
                control::write_frame(&mut stream, &response, control::MAX_RESPONSE)
                    .await
                    .unwrap();
            }
        });
        let response = transact(&config, &Request::Status {}, false).await.unwrap();
        assert_eq!(response.workspace_id, "one");
        assert_eq!(
            response.error.as_deref(),
            Some("Completion unknown; query status.")
        );
        let error = execute(&config, &Request::Status {}, false)
            .await
            .unwrap_err();
        assert_eq!(error, "Completion unknown; query status.");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn preparation_failure_is_distinct_from_a_missing_existing_workspace() {
        let (_endpoint, config, listener) = Endpoint::new();
        drop(listener);
        assert!(matches!(
            transact(&config, &Request::Status {}, false).await,
            Err(ClientError::Connection(_))
        ));
        fs::remove_file(&config).unwrap();
        assert!(matches!(
            transact(&config, &Request::Status {}, false).await,
            Err(ClientError::Preparation(_))
        ));
    }
}
