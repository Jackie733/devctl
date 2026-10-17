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
        read_frame::<Request>(&mut stream, MAX_REQUEST)
            .await
            .map(Some)
    })
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "Control request timed out."))??;
    let Some(request) = request else {
        return Ok(());
    };
    let (reply, response) = oneshot::channel();
    let mut response = match commands.try_send(Command { request, reply }) {
        Ok(()) => match timeout(Duration::from_secs(10), response).await {
            Ok(Ok(response)) => response,
            Ok(Err(_)) => {
                Response::failed("Workspace is shutting down; request completion is unknown.")
            }
            Err(_) => {
                Response::failed("Request completion timed out; query status before retrying.")
            }
        },
        Err(_) => Response::failed("Workspace is busy or shutting down; request was not accepted."),
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
}
