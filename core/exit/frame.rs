//! Exit protocol frame: ALPN, version, reply codes, building and parsing the request frame.
//! Pure functions without networking — `frame_tests.rs` tests them.

use std::time::Duration;

use anyhow::{bail, Context, Result};

/// ALPN of the exit protocol.
pub const EXIT_ALPN: &[u8] = b"meshtown/exit/1";
/// Version of the request and reply frame.
pub const EXIT_PROTOCOL_VERSION: u8 = 1;
/// Short reply: the exit has no free stream slot. Only the version byte is written and the stream is closed for
/// writing; clients that don't know it see an unanswered request and go on to the next exit.
pub const EXIT_BUSY_REPLY: [u8; 1] = [EXIT_PROTOCOL_VERSION];
pub const EXIT_TCP_CONNECT_TIMEOUT: Duration = Duration::from_secs(8);

/// The connection is open, bytes follow.
pub const EXIT_CODE_OK: u8 = 0;
/// The domain is not in the network's shared list.
pub const EXIT_CODE_NOT_IN_LIST: u8 = 1;
/// The domain's address was not obtained.
pub const EXIT_CODE_NO_ADDRESS: u8 = 2;
/// The connection to the domain didn't open.
pub const EXIT_CODE_CONNECT_FAILED: u8 = 3;
/// The port is not allowed (the rule was removed, but the code stays in the protocol: old exits in the network still
/// send it, and the requesting side prints its text).
pub const EXIT_CODE_PORT_NOT_ALLOWED: u8 = 4;
/// The member doesn't agree to be an exit.
pub const EXIT_CODE_REFUSED: u8 = 5;
/// The request could not be parsed.
pub const EXIT_CODE_BAD_REQUEST: u8 = 6;
/// The address is closed to exit: private network, loopback, link-local, multicast, service, or
/// the member's own address. Old clients print "unknown reply
/// code" for it and still treat it as a refusal.
pub const EXIT_CODE_ADDRESS_FORBIDDEN: u8 = 7;

/// Returns a human-readable name for a reply code.
pub fn exit_code_text(code: u8) -> &'static str {
    match code {
        EXIT_CODE_OK => "connection open",
        EXIT_CODE_NOT_IN_LIST => "the domain is not in the shared list of the Mesh Town network",
        EXIT_CODE_NO_ADDRESS => "the domain's address was not obtained",
        EXIT_CODE_CONNECT_FAILED => "the connection to the domain didn't open",
        EXIT_CODE_PORT_NOT_ALLOWED => "port not allowed",
        EXIT_CODE_REFUSED => "this member doesn't act as an exit for you",
        EXIT_CODE_BAD_REQUEST => "the request could not be parsed",
        EXIT_CODE_ADDRESS_FORBIDDEN => {
            "the address is closed to exit: local or service network, or the member's own address"
        }
        _ => "unknown reply code",
    }
}

/// Builds a request frame: version, name length, name, port (high byte first).
pub fn encode_exit_request(target_host: &str, port: u16) -> Result<Vec<u8>> {
    let bytes = target_host.as_bytes();
    if bytes.is_empty() || bytes.len() > 255 {
        bail!(
            "the domain name doesn't fit in the request frame: {} bytes",
            bytes.len()
        );
    }
    let mut out = Vec::with_capacity(4 + bytes.len());
    out.push(EXIT_PROTOCOL_VERSION);
    out.push(bytes.len() as u8);
    out.extend_from_slice(bytes);
    out.extend_from_slice(&port.to_be_bytes());
    Ok(out)
}

/// Parses a request frame. A pure function: no networking inside.
pub fn parse_exit_request(buf: &[u8]) -> Result<(String, u16)> {
    if buf.len() < 4 {
        bail!(
            "the request frame is shorter than four bytes: {}",
            buf.len()
        );
    }
    if buf[0] != EXIT_PROTOCOL_VERSION {
        bail!("unsupported request frame version: {}", buf[0]);
    }
    let len = usize::from(buf[1]);
    if len == 0 {
        bail!("the request frame has an empty domain name");
    }
    if buf.len() != 2 + len + 2 {
        bail!(
            "the request frame length doesn't match: {} bytes with a name of {} bytes",
            buf.len(),
            len
        );
    }
    let name = std::str::from_utf8(&buf[2..2 + len]).context("domain name is not UTF-8")?;
    let port = u16::from_be_bytes([buf[2 + len], buf[3 + len]]);
    Ok((name.to_ascii_lowercase(), port))
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
