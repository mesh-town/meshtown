//! Serving an answer page on an in-memory stream.

use std::time::Duration;

use tokio::io::{duplex, split, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::page::{render_page, MeshPage};
use crate::transport::net::{BoxRecv, BoxSend};

/// How long a client may take to send the request head.
pub(crate) const PAGE_HEAD_TIMEOUT: Duration = Duration::from_secs(5);

const MAX_HEAD: usize = 16 * 1024;
const TLS_HANDSHAKE: u8 = 0x16;

/// Returns the client side of a stream answered with `page`: bytes written to
/// the send half are read as a request, the response comes out of the recv half.
pub(crate) fn serve_page(page: MeshPage) -> (BoxSend, BoxRecv) {
    let (client, server) = duplex(MAX_HEAD * 4);
    tokio::spawn(answer(server, page));
    let (read, write) = split(client);
    (Box::new(write), Box::new(read))
}

async fn answer<S: AsyncRead + AsyncWrite + Unpin>(mut stream: S, page: MeshPage) {
    let head = tokio::time::timeout(PAGE_HEAD_TIMEOUT, read_head(&mut stream)).await;
    let Ok(Some(head)) = head else { return };
    let reply = render_page(&page, head.starts_with(b"HEAD "));
    let _ = stream.write_all(&reply).await;
    let _ = stream.shutdown().await;
}

/// Reads up to the end of the request head; `None` for TLS, overflow or EOF.
async fn read_head<R: AsyncRead + Unpin>(stream: &mut R) -> Option<Vec<u8>> {
    let mut head = Vec::new();
    let mut chunk = [0u8; 2048];
    while head.len() < MAX_HEAD {
        let n = stream.read(&mut chunk).await.ok()?;
        if n == 0 || (head.is_empty() && chunk[0] == TLS_HANDSHAKE) {
            return None;
        }
        head.extend_from_slice(&chunk[..n]);
        if head.windows(4).any(|w| w == b"\r\n\r\n") {
            return Some(head);
        }
    }
    None
}

#[cfg(test)]
#[path = "page_serve_tests.rs"]
mod tests;
