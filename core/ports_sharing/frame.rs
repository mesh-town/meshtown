//! Frame of the `meshtown/port/1` protocol: ALPN, version, reply codes, building and parsing frames.
//! Pure functions without a network — checked by `frame_tests.rs`.

use anyhow::{bail, Result};

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

/// Request frame: version and port (most significant byte first). The port is not validated.
pub fn encode_port_request(port: u16) -> [u8; 3] {
    let p = port.to_be_bytes();
    [PORT_FRAME_VERSION, p[0], p[1]]
}

/// Parsing a request frame: a length other than 3, a foreign version and port 0 are errors.
pub fn parse_port_request(buf: &[u8]) -> Result<u16> {
    if buf.len() != 3 {
        bail!("request frame is not three bytes: {}", buf.len());
    }
    if buf[0] != PORT_FRAME_VERSION {
        bail!("foreign request frame version: {}", buf[0]);
    }
    let port = u16::from_be_bytes([buf[1], buf[2]]);
    if port == 0 {
        bail!("port 0 in the request frame");
    }
    Ok(port)
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
        _ => "unknown reply code",
    }
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
