//! Wire format: the DHT connection header, the accepting side's reply, control stream frames.
//! Everything is big-endian. A format error is `io::ErrorKind::InvalidData`; a break in the middle of a frame is `UnexpectedEof`.

use std::io;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// First four bytes of any header; foreign ones — the connection is dropped without a reply.
pub const MAGIC: [u8; 4] = *b"mvh1";
/// How long the accepting side waits for a header and the dialing side for a reply.
pub const HEADER_LIMIT: Duration = Duration::from_secs(5);
/// Refusal: there is no such service (same value as `UNSERVED_CODE` of the facade).
pub const REFUSE_UNSERVED: u32 = 1;
pub const REFUSE_NO_SESSION: u32 = 2;
pub const REFUSE_SESSION_CLOSED: u32 = 3;
/// Refusal: the peer is not a member of the network for this service.
pub const REFUSE_NOT_MEMBER: u32 = 4;
pub const SERVICE_NAME_MAX_BYTES: usize = 64;
pub const REASON_MAX_BYTES: usize = 255;

const KIND_OPEN: u8 = 1;
const KIND_STREAM: u8 = 2;
const REPLY_ACK: u8 = 0;
const REPLY_REFUSE: u8 = 1;
const CTRL_CLOSE: u8 = 0x10;
const CTRL_PING: u8 = 0x11;
const CTRL_PONG: u8 = 0x12;

/// Whose number names the session of a data stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    /// The number was chosen by whoever dials this stream (byte 0).
    Dialer,
    /// The number was chosen by the accepting side (byte 1).
    Acceptor,
}

/// Header: the first bytes of a DHT connection from the dialing side.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Header {
    Open { session: u64, service_name: Vec<u8> },
    Stream { session: u64, owner: Owner },
}

/// Reply of the accepting side to a header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reply {
    Ack,
    Refuse { code: u32, reason: Vec<u8> },
}

/// Control stream frames after `Ack`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ctrl {
    Close { code: u32, reason: Vec<u8> },
    Ping,
    Pong,
}

fn bad(text: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, text.to_string())
}

fn len_as_u8_checked(bytes: &[u8], max: usize, what: &str) -> io::Result<u8> {
    if bytes.len() > max {
        return Err(bad(&format!("{what} longer than {max} bytes")));
    }
    Ok(bytes.len() as u8)
}

async fn read_u8<R: AsyncRead + Unpin>(r: &mut R) -> io::Result<u8> {
    r.read_u8().await
}

async fn read_len_prefixed<R: AsyncRead + Unpin>(
    r: &mut R,
    max: usize,
    what: &str,
) -> io::Result<Vec<u8>> {
    let len = read_u8(r).await? as usize;
    if len > max {
        return Err(bad(&format!("{what} longer than {max} bytes")));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).await?;
    Ok(buf)
}

async fn write_len_prefixed<W: AsyncWrite + Unpin>(
    w: &mut W,
    bytes: &[u8],
    max: usize,
    what: &str,
) -> io::Result<()> {
    let len = len_as_u8_checked(bytes, max, what)?;
    w.write_u8(len).await?;
    w.write_all(bytes).await
}

pub async fn write_header<W: AsyncWrite + Unpin>(w: &mut W, h: &Header) -> io::Result<()> {
    let mut buf = Vec::with_capacity(16);
    buf.extend_from_slice(&MAGIC);
    match h {
        Header::Open {
            session,
            service_name,
        } => {
            let len = len_as_u8_checked(service_name, SERVICE_NAME_MAX_BYTES, "service name")?;
            buf.push(KIND_OPEN);
            buf.extend_from_slice(&session.to_be_bytes());
            buf.push(len);
            buf.extend_from_slice(service_name);
        }
        Header::Stream { session, owner } => {
            buf.push(KIND_STREAM);
            buf.extend_from_slice(&session.to_be_bytes());
            buf.push(matches!(owner, Owner::Acceptor) as u8);
        }
    }
    w.write_all(&buf).await
}

pub async fn read_header<R: AsyncRead + Unpin>(r: &mut R) -> io::Result<Header> {
    let mut magic = [0u8; 4];
    r.read_exact(&mut magic).await?;
    if magic != MAGIC {
        return Err(bad("foreign header"));
    }
    match read_u8(r).await? {
        KIND_OPEN => read_open(r).await,
        KIND_STREAM => read_stream(r).await,
        _ => Err(bad("unknown header kind")),
    }
}

async fn read_open<R: AsyncRead + Unpin>(r: &mut R) -> io::Result<Header> {
    let session = r.read_u64().await?;
    let service_name = read_len_prefixed(r, SERVICE_NAME_MAX_BYTES, "service name").await?;
    Ok(Header::Open {
        session,
        service_name,
    })
}

async fn read_stream<R: AsyncRead + Unpin>(r: &mut R) -> io::Result<Header> {
    let session = r.read_u64().await?;
    let owner = match read_u8(r).await? {
        0 => Owner::Dialer,
        1 => Owner::Acceptor,
        _ => return Err(bad("unknown session owner")),
    };
    Ok(Header::Stream { session, owner })
}

pub async fn write_reply<W: AsyncWrite + Unpin>(w: &mut W, reply: &Reply) -> io::Result<()> {
    match reply {
        Reply::Ack => w.write_u8(REPLY_ACK).await,
        Reply::Refuse { code, reason } => write_coded(w, REPLY_REFUSE, *code, reason).await,
    }
}

pub async fn read_reply<R: AsyncRead + Unpin>(r: &mut R) -> io::Result<Reply> {
    match read_u8(r).await? {
        REPLY_ACK => Ok(Reply::Ack),
        REPLY_REFUSE => {
            let (code, reason) = read_coded(r).await?;
            Ok(Reply::Refuse { code, reason })
        }
        _ => Err(bad("unknown reply")),
    }
}

async fn write_coded<W: AsyncWrite + Unpin>(
    w: &mut W,
    kind: u8,
    code: u32,
    reason: &[u8],
) -> io::Result<()> {
    len_as_u8_checked(reason, REASON_MAX_BYTES, "reason")?;
    w.write_u8(kind).await?;
    w.write_u32(code).await?;
    write_len_prefixed(w, reason, REASON_MAX_BYTES, "reason").await
}

async fn read_coded<R: AsyncRead + Unpin>(r: &mut R) -> io::Result<(u32, Vec<u8>)> {
    let code = r.read_u32().await?;
    Ok((
        code,
        read_len_prefixed(r, REASON_MAX_BYTES, "reason").await?,
    ))
}

pub async fn write_ctrl<W: AsyncWrite + Unpin>(w: &mut W, frame: &Ctrl) -> io::Result<()> {
    match frame {
        Ctrl::Close { code, reason } => write_coded(w, CTRL_CLOSE, *code, reason).await,
        Ctrl::Ping => w.write_u8(CTRL_PING).await,
        Ctrl::Pong => w.write_u8(CTRL_PONG).await,
    }
}

pub async fn read_ctrl<R: AsyncRead + Unpin>(r: &mut R) -> io::Result<Ctrl> {
    match read_u8(r).await? {
        CTRL_CLOSE => {
            let (code, reason) = read_coded(r).await?;
            Ok(Ctrl::Close { code, reason })
        }
        CTRL_PING => Ok(Ctrl::Ping),
        CTRL_PONG => Ok(Ctrl::Pong),
        _ => Err(bad("unknown control frame")),
    }
}

#[cfg(test)]
#[path = "wire_tests.rs"]
mod tests;
