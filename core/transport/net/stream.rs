//! Common async and stream types: `BoxFuture` and the stream half pair `StreamPair`.
//! Only declarations here, no behavior.

use std::future::Future;
use std::pin::Pin;
use tokio::io::{AsyncRead, AsyncWrite};

/// A boxed future: our own alias instead of the `futures` and `async-trait` crates (object-safe traits).
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Sending half of a stream; `shutdown` = half-close (the peer will see the end of data).
pub type BoxSend = Box<dyn AsyncWrite + Send + Unpin>;
/// Receiving half of a stream.
pub type BoxRecv = Box<dyn AsyncRead + Send + Unpin>;

/// Bidirectional stream: a pair of halves.
pub struct StreamPair {
    pub send: BoxSend,
    pub recv: BoxRecv,
}

/// Reads the stream to the end, but no more than `limit` bytes: more gives `InvalidData`, what was read
/// is discarded. Holds at most `limit + 1` bytes in memory; does not close the stream itself.
pub async fn read_to_end_limited(recv: &mut BoxRecv, limit: usize) -> std::io::Result<Vec<u8>> {
    use tokio::io::AsyncReadExt;
    let mut out = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let room = (limit + 1 - out.len()).min(chunk.len());
        let n = recv.read(&mut chunk[..room]).await?;
        out.extend_from_slice(&chunk[..n]);
        if out.len() > limit {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        if n == 0 {
            return Ok(out);
        }
    }
}

#[cfg(test)]
#[path = "stream_tests.rs"]
mod limited_tests;
#[cfg(test)]
#[path = "traits_tests.rs"]
mod tests;
