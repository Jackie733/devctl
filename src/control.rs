use std::{collections::BTreeMap, io, os::unix::ffi::OsStrExt, path::Path, time::Duration};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
    sync::{mpsc, oneshot},
    time::timeout,
};

use crate::{logs::Batch, manager::ServiceStatus, probe};

pub const CONTROL_HEADER: &[u8; 9] = b"CONTROL1\n";
pub const MAX_REQUEST: usize = 64 * 1024;
pub const MAX_RESPONSE: usize = 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Status {},
    ReadLogs {
        query: crate::archive::Query,
    },
    LogSessions {},
    Start {
        targets: Vec<String>,
        all: bool,
    },
    Stop {
        targets: Vec<String>,
        all: bool,
    },
    Shutdown {},
    /// Reviewed append-only save, applied by the workspace owner.
    AddProject {
        workspace_id: String,
        proposal: crate::import::Proposal,
    },
    Logs {
        service: String,
        limit: usize,
        after: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        before: Option<u64>,
    },
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    #[serde(default)]
    pub workspace_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub services: BTreeMap<String, ServiceStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logs: Option<Batch>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sessions: Vec<crate::archive::Session>,
}

impl Response {
    pub fn failed(error: impl Into<String>) -> Self {
        Self {
            error: Some(error.into()),
            ..Self::default()
        }
    }
}

pub struct Command {
    pub request: Request,
    pub reply: oneshot::Sender<Response>,
}

fn invalid(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

pub async fn read_frame<T: DeserializeOwned>(
    stream: &mut UnixStream,
    limit: usize,
) -> io::Result<T> {
    let length = stream.read_u32().await? as usize;
    if length > limit {
        return Err(invalid("Control message exceeds size limit."));
    }
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes).await?;
    let value = std::str::from_utf8(&bytes).map_err(invalid)?;
    toml::from_str(value).map_err(invalid)
}

pub async fn write_frame<T: Serialize>(
    stream: &mut UnixStream,
    value: &T,
    limit: usize,
) -> io::Result<()> {
    let text = toml::to_string(value).map_err(invalid)?;
    if text.len() > limit {
        return Err(invalid("Control message exceeds size limit."));
    }
    stream.write_u32(text.len() as u32).await?;
    stream.write_all(text.as_bytes()).await
}

pub async fn verify_identity(stream: &mut UnixStream, config: &Path) -> io::Result<()> {
    let mut header = [0; 9];
    stream.read_exact(&mut header).await?;
    if &header != b"DEVCTL 1\n" {
        return Err(invalid("Incompatible control protocol."));
    }
    let length = stream.read_u32().await? as usize;
    if length > 16 * 1024 {
        return Err(invalid("Config identity exceeds size limit."));
    }
    let mut identity = vec![0; length];
    stream.read_exact(&mut identity).await?;
    if identity != config.as_os_str().as_bytes() {
        return Err(invalid(
            "Control endpoint belongs to a different configuration.",
        ));
    }
    Ok(())
}

pub async fn connection(
    mut stream: UnixStream,
    config: &Path,
    commands: mpsc::Sender<Command>,
    workspace_id: &str,
) -> io::Result<()> {
    let request = timeout(Duration::from_secs(2), async {
        let mut header = [0; 9];
        stream.read_exact(&mut header).await?;
        if &header == b"DEVCTL 1\n" {
            probe::write_identity(&mut stream, config).await?;
            return Ok(None);
        }
        if &header != CONTROL_HEADER {
            return Err(invalid("Unsupported control protocol."));
        }
        probe::write_identity(&mut stream, config).await?;
        Ok(Some(read_frame::<Request>(&mut stream, MAX_REQUEST).await))
    })
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "Control request timed out."))??;
    let Some(request) = request else {
        return Ok(());
    };
    let (reply, response) = oneshot::channel();
    let mut response = match request {
        Err(error) => Response::failed(format!(
            "Control request was rejected before execution: {error}"
        )),
        Ok(Request::AddProject {
            workspace_id: expected,
            ..
        }) if expected != workspace_id => {
            Response::failed("Workspace changed; review the addition again.")
        }
        Ok(request) => match commands.try_send(Command { request, reply }) {
            Ok(()) => match timeout(Duration::from_secs(10), response).await {
                Ok(Ok(response)) => response,
                Ok(Err(_)) => {
                    Response::failed("Workspace is shutting down; request completion is unknown.")
                }
                Err(_) => {
                    Response::failed("Request completion timed out; query status before retrying.")
                }
            },
            Err(_) => {
                Response::failed("Workspace is busy or shutting down; request was not accepted.")
            }
        },
    };
    response.workspace_id = workspace_id.to_owned();
    timeout(Duration::from_secs(2), async {
        let response = if toml::to_string(&response).map_err(invalid)?.len() > MAX_RESPONSE {
            Response {
                workspace_id: workspace_id.to_owned(),
                ..Response::failed("Response exceeds size limit.")
            }
        } else {
            response
        };
        write_frame(&mut stream, &response, MAX_RESPONSE).await?;
        stream.shutdown().await
    })
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "Control response timed out."))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_protocol_keeps_old_requests_compatible_and_never_silently_drops_backward_bounds() {
        let old = "action='logs'\nservice='web'\nlimit=10\n";
        assert!(matches!(
            toml::from_str::<Request>(old).unwrap(),
            Request::Logs { before: None, .. }
        ));
        let forward = toml::to_string(&Request::Logs {
            service: "web".into(),
            limit: 10,
            after: Some(3),
            before: None,
        })
        .unwrap();
        assert!(!forward.contains("before"));
        let backward = toml::to_string(&Request::Logs {
            service: "web".into(),
            limit: 10,
            after: None,
            before: Some(4),
        })
        .unwrap();
        assert!(backward.contains("before = 4"));
        let old_batch = "records=[]\ndropped=0\nincomplete=false\ngap=false\nlatest=0\n";
        let batch: Batch = toml::from_str(old_batch).unwrap();
        assert!(batch.oldest.is_none() && !batch.has_older);
    }

    #[tokio::test]
    async fn framing_rejects_oversized_lengths_before_reading_payload() {
        let (mut client, mut server) = UnixStream::pair().unwrap();
        client.write_u32((MAX_REQUEST + 1) as u32).await.unwrap();
        assert_eq!(
            read_frame::<Request>(&mut server, MAX_REQUEST)
                .await
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[tokio::test]
    async fn request_frames_keep_arguments_and_reject_unknown_fields() {
        let (mut client, mut server) = UnixStream::pair().unwrap();
        let value = Request::Start {
            targets: vec!["two words".into(), "web".into()],
            all: false,
        };
        write_frame(&mut client, &value, MAX_REQUEST).await.unwrap();
        assert!(
            matches!(read_frame::<Request>(&mut server, MAX_REQUEST).await.unwrap(), Request::Start { targets, all: false } if targets == ["two words", "web"])
        );
        let payload = b"action = 'status'\nunknown = 1\n";
        client.write_u32(payload.len() as u32).await.unwrap();
        client.write_all(payload).await.unwrap();
        assert!(
            read_frame::<Request>(&mut server, MAX_REQUEST)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn identity_mismatch_is_rejected_before_sending_a_control_request() {
        let (mut client, mut server) = UnixStream::pair().unwrap();
        probe::write_identity(&mut server, Path::new("/tmp/other"))
            .await
            .unwrap();
        let error = verify_identity(&mut client, Path::new("/tmp/expected"))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("different configuration"));
    }
    #[tokio::test]
    async fn addition_refuses_a_stale_workspace_before_dispatch() {
        let (mut client, server) = UnixStream::pair().unwrap();
        let (sender, mut receiver) = mpsc::channel(1);
        let task = tokio::spawn(async move {
            connection(server, Path::new("/tmp/config"), sender, "current").await
        });
        client.write_all(CONTROL_HEADER).await.unwrap();
        let mut header = [0; 9];
        client.read_exact(&mut header).await.unwrap();
        assert_eq!(&header, b"DEVCTL 1\n");
        let length = client.read_u32().await.unwrap();
        let mut identity = vec![0; length as usize];
        client.read_exact(&mut identity).await.unwrap();
        write_frame(
            &mut client,
            &toml::from_str::<Request>("action='add_project'\nworkspace_id='stale'\n[proposal]\ndirectory='/tmp/project'\narguments=['sleep','60']\nrevision='old'\nproposed='reviewed'\nid='project'\n").unwrap(),
            MAX_REQUEST,
        )
        .await
        .unwrap();
        let response: Response = read_frame(&mut client, MAX_RESPONSE).await.unwrap();
        assert_eq!(response.workspace_id, "current");
        assert!(response.error.unwrap().contains("Workspace changed"));
        task.await.unwrap().unwrap();
        assert!(
            receiver.try_recv().is_err(),
            "Stale addition must never reach the manager"
        );
    }

    #[tokio::test]
    async fn invalid_requests_return_explicit_errors_without_dispatch() {
        for payload in [
            "action='future_action'\n",
            "action='status'\nunknown=1\n",
            "action='add_project'\nworkspace_id='current'\n",
        ] {
            let (mut client, server) = UnixStream::pair().unwrap();
            let (sender, mut receiver) = mpsc::channel(1);
            let task = tokio::spawn(async move {
                connection(server, Path::new("/tmp/config"), sender, "current").await
            });
            client.write_all(CONTROL_HEADER).await.unwrap();
            verify_identity(&mut client, Path::new("/tmp/config"))
                .await
                .unwrap();
            client.write_u32(payload.len() as u32).await.unwrap();
            client.write_all(payload.as_bytes()).await.unwrap();
            let response: Response = read_frame(&mut client, MAX_RESPONSE).await.unwrap();
            assert_eq!(response.workspace_id, "current");
            assert!(
                response
                    .error
                    .unwrap()
                    .contains("rejected before execution")
            );
            task.await.unwrap().unwrap();
            assert!(
                receiver.try_recv().is_err(),
                "Invalid requests must not execute"
            );
        }
    }
}
