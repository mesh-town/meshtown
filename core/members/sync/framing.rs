//! The common stream frame: u32 BE length + postcard. Used by `flood::wire` and `replica::session`.

use crate::transport::net::error::{NetError, NetResult};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::{Error, ErrorKind};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

fn invalid(what: &str) -> NetError {
    NetError::Io(Error::new(ErrorKind::InvalidData, what.to_string()))
}

/// A body longer than `max` is `Io(InvalidData)`.
pub(crate) async fn write_framed<W, T>(send: &mut W, msg: &T, max: usize) -> NetResult<()>
where
    W: AsyncWrite + Unpin,
    T: Serialize,
{
    let body = postcard::to_stdvec(msg).map_err(|_| invalid("frame cannot be serialized"))?;
    if body.len() > max {
        return Err(invalid("frame longer than the limit"));
    }
    let mut bytes = (body.len() as u32).to_be_bytes().to_vec();
    bytes.extend_from_slice(&body);
    send.write_all(&bytes).await?;
    send.flush().await?;
    Ok(())
}

/// A length above `max` or a corrupt body is `Io(InvalidData)`; end of stream is `Closed`.
pub(crate) async fn read_framed<R, T>(recv: &mut R, max: usize) -> NetResult<T>
where
    R: AsyncRead + Unpin,
    T: DeserializeOwned,
{
    let mut head = [0u8; 4];
    read_exact(recv, &mut head).await?;
    let len = u32::from_be_bytes(head) as usize;
    if len > max {
        return Err(invalid("frame length above the limit"));
    }
    let mut body = vec![0u8; len];
    read_exact(recv, &mut body).await?;
    postcard::from_bytes(&body).map_err(|_| invalid("corrupt frame"))
}

async fn read_exact<R: AsyncRead + Unpin>(recv: &mut R, buf: &mut [u8]) -> NetResult<()> {
    match recv.read_exact(buf).await {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == ErrorKind::UnexpectedEof => Err(NetError::Closed),
        Err(e) => Err(NetError::Io(e)),
    }
}

#[cfg(test)]
#[path = "framing_tests.rs"]
mod tests;
