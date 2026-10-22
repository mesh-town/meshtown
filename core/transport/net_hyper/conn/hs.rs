//! Header and reply exchange on the control and data streams of a session.

use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net_hyper::wire::{read_reply, write_header, Header, Reply};
use crate::transport::net_hyper::wire::{
    HEADER_LIMIT, REFUSE_NOT_MEMBER, REFUSE_NO_SESSION, REFUSE_SESSION_CLOSED, REFUSE_UNSERVED,
};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::time::timeout;

pub(super) async fn send_header<W: AsyncWrite + Unpin>(
    w: &mut W,
    header: &Header,
) -> NetResult<()> {
    write_header(w, header).await.map_err(NetError::Io)?;
    w.flush().await.map_err(NetError::Io)
}

/// Waits for the accepting side's reply for no longer than `HEADER_LIMIT`.
pub(super) async fn expect_ack<R: AsyncRead + Unpin>(r: &mut R) -> NetResult<()> {
    let reply = timeout(HEADER_LIMIT, read_reply(r))
        .await
        .map_err(|_| NetError::Timeout)?;
    match reply.map_err(NetError::Io)? {
        Reply::Ack => Ok(()),
        Reply::Refuse { code, .. } => Err(refusal(code)),
    }
}

fn refusal(code: u32) -> NetError {
    match code {
        REFUSE_UNSERVED | REFUSE_NOT_MEMBER => NetError::Refused,
        REFUSE_NO_SESSION | REFUSE_SESSION_CLOSED => NetError::Closed,
        other => NetError::Other(anyhow::anyhow!("peer refused, code {other}")),
    }
}
