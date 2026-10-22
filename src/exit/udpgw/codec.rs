//! Frame codec of the UDP gateway: `LEN(2) FLAGS(1) CONN_ID(2) [ATYP ADDR PORT] DATA`, big-endian.
//! The address is present only when the `DATA` flag is set and uses the SOCKS5 address layout.

use std::io::{Error, ErrorKind, Result};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use tokio::io::{AsyncRead, AsyncReadExt};

pub(super) const FLAG_KEEPALIVE: u8 = 0x01;
pub(super) const FLAG_DATA: u8 = 0x02;
pub(super) const FLAG_CLOSE: u8 = 0x04;
pub(super) const FLAG_ERR: u8 = 0x20;

const ATYP_IPV4: u8 = 1;
const ATYP_DOMAIN: u8 = 3;
const ATYP_IPV6: u8 = 4;
/// Flags (1) and connection id (2).
const HEADER_LEN: usize = 3;

/// Where a datagram goes (or where a reply came from).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Target {
    Ip(SocketAddr),
    Domain(String, u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Frame {
    pub flags: u8,
    pub conn_id: u16,
    pub target: Option<Target>,
    pub data: Vec<u8>,
}

impl Frame {
    pub fn keepalive(conn_id: u16) -> Frame {
        Frame { flags: FLAG_KEEPALIVE, conn_id, target: None, data: Vec::new() }
    }

    pub fn error(conn_id: u16) -> Frame {
        Frame { flags: FLAG_ERR, conn_id, target: None, data: Vec::new() }
    }

    pub fn data(conn_id: u16, from: SocketAddr, data: Vec<u8>) -> Frame {
        Frame { flags: FLAG_DATA, conn_id, target: Some(Target::Ip(from)), data }
    }
}

fn invalid(what: &str) -> Error {
    Error::new(ErrorKind::InvalidData, what.to_string())
}

/// Splits `n` bytes off the front of the slice.
fn take<'a>(buf: &mut &'a [u8], n: usize) -> Result<&'a [u8]> {
    if buf.len() < n {
        return Err(invalid("frame is cut short"));
    }
    let (head, tail) = buf.split_at(n);
    *buf = tail;
    Ok(head)
}

fn parse_target(buf: &mut &[u8]) -> Result<Target> {
    let atyp = take(buf, 1)?[0];
    let target = match atyp {
        ATYP_IPV4 => {
            let b = take(buf, 4)?;
            let ip = IpAddr::V4(Ipv4Addr::new(b[0], b[1], b[2], b[3]));
            (Some(ip), String::new())
        }
        ATYP_IPV6 => {
            let octets: [u8; 16] = take(buf, 16)?.try_into().map_err(|_| invalid("bad IPv6"))?;
            (Some(IpAddr::V6(Ipv6Addr::from(octets))), String::new())
        }
        ATYP_DOMAIN => {
            let len = usize::from(take(buf, 1)?[0]);
            let name = String::from_utf8_lossy(take(buf, len)?).to_ascii_lowercase();
            (None, name)
        }
        _ => return Err(invalid("unknown address type")),
    };
    let port = u16::from_be_bytes(take(buf, 2)?.try_into().map_err(|_| invalid("bad port"))?);
    Ok(match target {
        (Some(ip), _) => Target::Ip(SocketAddr::new(ip, port)),
        (None, name) => Target::Domain(name, port),
    })
}

/// Parses the frame body (everything after the length field).
pub(super) fn parse_body(body: &[u8]) -> Result<Frame> {
    let mut rest = body;
    let head = take(&mut rest, HEADER_LEN)?;
    let (flags, conn_id) = (head[0], u16::from_be_bytes([head[1], head[2]]));
    let target = if flags & FLAG_DATA == FLAG_DATA { Some(parse_target(&mut rest)?) } else { None };
    Ok(Frame { flags, conn_id, target, data: rest.to_vec() })
}

/// Reads one whole frame; an end of stream before the first byte is `UnexpectedEof`.
pub(super) async fn read_frame<R: AsyncRead + Unpin>(recv: &mut R) -> Result<Frame> {
    let mut len = [0u8; 2];
    recv.read_exact(&mut len).await?;
    let mut body = vec![0u8; usize::from(u16::from_be_bytes(len))];
    recv.read_exact(&mut body).await?;
    parse_body(&body)
}

fn push_target(out: &mut Vec<u8>, target: &Target) {
    match target {
        Target::Ip(SocketAddr::V4(a)) => {
            out.push(ATYP_IPV4);
            out.extend_from_slice(&a.ip().octets());
            out.extend_from_slice(&a.port().to_be_bytes());
        }
        Target::Ip(SocketAddr::V6(a)) => {
            out.push(ATYP_IPV6);
            out.extend_from_slice(&a.ip().octets());
            out.extend_from_slice(&a.port().to_be_bytes());
        }
        Target::Domain(name, port) => {
            out.push(ATYP_DOMAIN);
            out.push(u8::try_from(name.len()).unwrap_or(u8::MAX));
            out.extend_from_slice(&name.as_bytes()[..name.len().min(255)]);
            out.extend_from_slice(&port.to_be_bytes());
        }
    }
}

/// Builds the wire bytes of a frame; `None` if the body does not fit the 16-bit length field.
pub(super) fn encode(frame: &Frame) -> Option<Vec<u8>> {
    let mut body = vec![frame.flags];
    body.extend_from_slice(&frame.conn_id.to_be_bytes());
    if let Some(target) = &frame.target {
        push_target(&mut body, target);
    }
    body.extend_from_slice(&frame.data);
    let len = u16::try_from(body.len()).ok()?;
    let mut out = len.to_be_bytes().to_vec();
    out.extend_from_slice(&body);
    Some(out)
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
