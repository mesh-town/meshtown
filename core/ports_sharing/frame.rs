//! Frame of the `meshtown/port/1` protocol: ALPN, version, reply codes, building and parsing frames.
//! Pure functions without a network — checked by `frame_tests.rs`.

use anyhow::{bail, Result};

use super::types::Proto;

/// ALPN of the port publication protocol.
pub const PORT_ALPN: &[u8] = b"meshtown/port/1";
/// Version of the request and reply frame.
pub const PORT_FRAME_VERSION: u8 = 1;

/// The connection is open, bytes follow.
pub const PORT_CODE_OK: u8 = 0;
/// The member does not publish this port.
pub const PORT_CODE_NOT_PUBLISHED: u8 = 1;
/// The port on the member's computer does not respond.
pub const PORT_CODE_CONNECT_FAILED: u8 = 3;
/// The member does not open this port for the requester.
pub const PORT_CODE_REFUSED: u8 = 5;
/// The request could not be parsed.
pub const PORT_CODE_BAD_REQUEST: u8 = 6;
/// The publishing node serves too many UDP sessions already.
pub const PORT_CODE_BUSY: u8 = 7;
/// Version of a request frame that carries the protocol byte.
pub const PORT_FRAME_VERSION_PROTO: u8 = 2;

/// A request: the protocol and the port of the publishing node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PortRequest {
    pub proto: Proto,
    pub port: u16,
}

/// Request frame, the port most significant byte first: TCP is `[1, hi, lo]` (understood by every node),
/// UDP is `[2, proto, hi, lo]`. The port is not validated.
pub fn encode_port_request(req: PortRequest) -> Vec<u8> {
    let p = req.port.to_be_bytes();
    if req.proto.is_tcp() {
        return vec![PORT_FRAME_VERSION, p[0], p[1]];
    }
    vec![PORT_FRAME_VERSION_PROTO, req.proto.wire(), p[0], p[1]]
}

/// How many bytes follow the first three of a request: 1 for version 2, otherwise 0.
pub fn request_tail_len(head: &[u8; 3]) -> usize {
    usize::from(head[0] == PORT_FRAME_VERSION_PROTO)
}

/// Parsing a request frame: version 1 is three bytes (TCP), version 2 is four bytes; any other
/// version or length, an unknown protocol byte and port 0 are errors.
pub fn parse_port_request(buf: &[u8]) -> Result<PortRequest> {
    match (buf.first(), buf.len()) {
        (Some(&PORT_FRAME_VERSION), 3) => request_of(Proto::Tcp, [buf[1], buf[2]]),
        (Some(&PORT_FRAME_VERSION_PROTO), 4) => request_v2(buf),
        (Some(&v), n) if v == PORT_FRAME_VERSION || v == PORT_FRAME_VERSION_PROTO => {
            bail!("request frame of version {v} has a wrong length: {n}")
        }
        (Some(&v), _) => bail!("foreign request frame version: {v}"),
        (None, _) => bail!("empty request frame"),
    }
}

fn request_v2(buf: &[u8]) -> Result<PortRequest> {
    let Some(proto) = Proto::from_wire(buf[1]) else {
        bail!("unknown protocol byte in the request frame: {}", buf[1]);
    };
    request_of(proto, [buf[2], buf[3]])
}

fn request_of(proto: Proto, port: [u8; 2]) -> Result<PortRequest> {
    let port = u16::from_be_bytes(port);
    if port == 0 {
        bail!("port 0 in the request frame");
    }
    Ok(PortRequest { proto, port })
}

/// Reply frame: version and code.
pub fn encode_port_reply(code: u8) -> [u8; 2] {
    [PORT_FRAME_VERSION, code]
}

/// Parsing a reply frame; any code, even an unknown one, is `Ok`: anything other than `PORT_CODE_OK` is a refusal.
pub fn parse_port_reply(buf: &[u8]) -> Result<u8> {
    if buf.len() != 2 {
        bail!("reply frame is not two bytes: {}", buf.len());
    }
    if buf[0] != PORT_FRAME_VERSION {
        bail!("foreign reply frame version: {}", buf[0]);
    }
    Ok(buf[1])
}

/// Human-readable name of the reply code.
pub fn port_code_text(code: u8) -> &'static str {
    match code {
        PORT_CODE_OK => "connection open",
        PORT_CODE_NOT_PUBLISHED => "the port is not published",
        PORT_CODE_CONNECT_FAILED => "the port does not respond",
        PORT_CODE_REFUSED => "access to this port is not allowed",
        PORT_CODE_BAD_REQUEST => "request could not be parsed",
        PORT_CODE_BUSY => "too many UDP sessions",
        _ => "unknown reply code",
    }
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
