use std::{
    io::{self, ErrorKind},
    os::unix::ffi::OsStrExt,
    path::Path,
};

use tokio::{io::AsyncWriteExt, net::UnixStream};

#[cfg(test)]
use std::time::Duration;
#[cfg(test)]
use tokio::{io::AsyncReadExt, time::timeout};

const HEADER: &[u8; 9] = b"DEVCTL 1\n";
const MAX_IDENTITY_BYTES: usize = 16 * 1024;

#[cfg(test)]
pub async fn respond(mut stream: UnixStream, config_path: &Path) -> io::Result<()> {
    let identity = config_path.as_os_str().as_bytes();
    if identity.len() > MAX_IDENTITY_BYTES {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "Config identity is too long.",
        ));
    }

    timeout(Duration::from_secs(2), async {
        let mut header = [0; HEADER.len()];
        stream.read_exact(&mut header).await?;
        if &header != HEADER {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "Unsupported probe request or protocol version.",
            ));
        }

        write_identity(&mut stream, config_path).await?;
        stream.shutdown().await
    })
    .await
    .map_err(|_| io::Error::new(ErrorKind::TimedOut, "Probe timed out."))?
}

pub async fn write_identity(stream: &mut UnixStream, config_path: &Path) -> io::Result<()> {
    let identity = config_path.as_os_str().as_bytes();
    if identity.len() > MAX_IDENTITY_BYTES {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "Config identity is too long.",
        ));
    }
    stream.write_all(HEADER).await?;
    stream
        .write_all(&(identity.len() as u32).to_be_bytes())
        .await?;
    stream.write_all(identity).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{ffi::OsString, os::unix::ffi::OsStringExt, path::PathBuf};

    #[tokio::test]
    async fn fragmented_request_returns_a_framed_identity_without_utf8_loss() {
        let identity = b"/tmp/a path-\xff.toml";
        let path = PathBuf::from(OsString::from_vec(identity.to_vec()));
        let (mut client, server) = UnixStream::pair().unwrap();
        let task = tokio::spawn(async move { respond(server, &path).await });
        client.write_all(b"DEV").await.unwrap();
        tokio::task::yield_now().await;
        client.write_all(b"CTL 1\n").await.unwrap();
        let mut response = Vec::new();
        timeout(Duration::from_secs(3), client.read_to_end(&mut response))
            .await
            .unwrap()
            .unwrap();
        let mut expected = b"DEVCTL 1\n".to_vec();
        expected.extend_from_slice(&(identity.len() as u32).to_be_bytes());
        expected.extend_from_slice(identity);
        assert_eq!(response, expected);
        task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn wrong_version_and_invalid_header_are_rejected_without_a_success_reply() {
        for request in [b"DEVCTL 2\n", b"garbage!\n"] {
            let (mut client, server) = UnixStream::pair().unwrap();
            let task = tokio::spawn(async move { respond(server, Path::new("/tmp/config")).await });
            client.write_all(request).await.unwrap();
            assert_eq!(
                task.await.unwrap().unwrap_err().kind(),
                ErrorKind::InvalidData
            );
            let mut response = Vec::new();
            client.read_to_end(&mut response).await.unwrap();
            assert!(response.is_empty());
        }
    }

    #[tokio::test]
    async fn an_incomplete_request_and_a_disconnected_client_are_io_errors() {
        for request in [b"DEV".as_slice(), b""] {
            let (mut client, server) = UnixStream::pair().unwrap();
            client.write_all(request).await.unwrap();
            client.shutdown().await.unwrap();
            let error = respond(server, Path::new("/tmp/config")).await.unwrap_err();
            assert_eq!(error.kind(), ErrorKind::UnexpectedEof);
        }
    }

    #[tokio::test]
    async fn a_silent_client_has_a_deadline() {
        let (_client, server) = UnixStream::pair().unwrap();
        let error = timeout(
            Duration::from_secs(3),
            respond(server, Path::new("/tmp/config")),
        )
        .await
        .unwrap()
        .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::TimedOut);
    }

    #[tokio::test]
    async fn an_excessive_identity_is_rejected_before_reading_a_request() {
        let (_client, server) = UnixStream::pair().unwrap();
        let path = "x".repeat(16 * 1024 + 1);
        let error = respond(server, Path::new(&path)).await.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidData);
    }
}
