//! SOCKS5 frames (RFC 1928): codes, parsing the client's request and building the reply. Pure functions —
//! no network, they are checked by `frame_tests.rs`.

use std::fmt;
use std::net::Ipv4Addr;

use anyhow::{bail, Result};

/// The protocol version.
pub const VERSION: u8 = 5;
/// The "no authentication" method.
pub const METHOD_NO_AUTH: u8 = 0x00;
pub const METHOD_NO_ACCEPTABLE: u8 = 0xFF;
/// The "connect" command.
pub const CMD_CONNECT: u8 = 1;
/// Address type: IPv4.
pub const ATYP_IPV4: u8 = 1;
/// Address type: domain name.
pub const ATYP_DOMAIN: u8 = 3;
/// Address type: IPv6.
pub const ATYP_IPV6: u8 = 4;

/// The reply code "succeeded".
pub const REP_OK: u8 = 0;
/// The reply code "proxy refused".
pub const REP_GENERAL: u8 = 1;
/// The reply code "host unreachable" — our only refusal.
pub const REP_HOST_UNREACHABLE: u8 = 4;
/// The reply code "command not supported".
pub const REP_CMD_NOT_SUPPORTED: u8 = 7;
/// The reply code "address type not supported".
pub const REP_ATYP_NOT_SUPPORTED: u8 = 8;

/// The domain name length in the frame is written as one byte.
pub const MAX_NAME_LEN: usize = 255;

/// Where the connection is requested to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Domain(String),
    Ip(Ipv4Addr),
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Target::Domain(d) => write!(f, "{d}"),
            Target::Ip(ip) => write!(f, "{ip}"),
        }
    }
}

impl Target {
    /// A target from a text node name (HTTP proxy): an IPv4 literal is `Ip`, a name is `Domain` in lower
    /// case. Empty, longer than `MAX_NAME_LEN`, or IPv6 (`[..]` or with `:`) is `None`.
    pub fn from_host(host: &str) -> Option<Target> {
        if host.is_empty() || host.len() > MAX_NAME_LEN || host.contains(':') || host.starts_with('[') {
            return None;
        }
        Some(match host.parse::<Ipv4Addr>() {
            Ok(ip) => Target::Ip(ip),
            Err(_) => Target::Domain(host.to_lowercase()),
        })
    }
}

/// A parsed connect request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectRequest {
    pub target: Target,
    pub port: u16,
}

/// Why the request was not accepted. Each reason has its own reply code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestError {
    /// Fewer bytes than the frame requires.
    Short,
    /// The version is not 5.
    Version(u8),
    /// The command is not `CONNECT`.
    Command(u8),
    /// The address type is not supported (including IPv6).
    AddressKind(u8),
    /// The domain name length is zero.
    EmptyName,
    /// The domain name is not UTF-8.
    BadName,
}

impl RequestError {
    /// The code sent to the client in the reply.
    pub fn reply_code(&self) -> u8 {
        match self {
            RequestError::Command(_) => REP_CMD_NOT_SUPPORTED,
            RequestError::AddressKind(_) => REP_ATYP_NOT_SUPPORTED,
            _ => REP_GENERAL,
        }
    }
}

impl fmt::Display for RequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RequestError::Short => write!(f, "request shorter than a SOCKS5 frame"),
            RequestError::Version(v) => write!(f, "version {v}, but 5 is required"),
            RequestError::Command(c) => write!(f, "command {c}: only CONNECT is supported"),
            RequestError::AddressKind(a) => write!(f, "address type {a} is not supported"),
            RequestError::EmptyName => write!(f, "empty domain name"),
            RequestError::BadName => write!(f, "domain name is not UTF-8"),
        }
    }
}

impl std::error::Error for RequestError {}

/// Parses a connect request. A pure function: there is no network inside.
pub fn parse_socks5_request(buf: &[u8]) -> std::result::Result<ConnectRequest, RequestError> {
    if buf.len() < 4 {
        return Err(RequestError::Short);
    }
    if buf[0] != VERSION {
        return Err(RequestError::Version(buf[0]));
    }
    if buf[1] != CMD_CONNECT {
        return Err(RequestError::Command(buf[1]));
    }
    let (target, rest) = match buf[3] {
        ATYP_IPV4 => {
            if buf.len() < 10 {
                return Err(RequestError::Short);
            }
            let ip = Ipv4Addr::new(buf[4], buf[5], buf[6], buf[7]);
            (Target::Ip(ip), &buf[8..10])
        }
        ATYP_DOMAIN => {
            if buf.len() < 5 {
                return Err(RequestError::Short);
            }
            let len = usize::from(buf[4]);
            if len == 0 {
                return Err(RequestError::EmptyName);
            }
            if buf.len() < 5 + len + 2 {
                return Err(RequestError::Short);
            }
            let name =
                std::str::from_utf8(&buf[5..5 + len]).map_err(|_| RequestError::BadName)?;
            (
                Target::Domain(name.to_ascii_lowercase()),
                &buf[5 + len..5 + len + 2],
            )
        }
        other => return Err(RequestError::AddressKind(other)),
    };
    Ok(ConnectRequest {
        target,
        port: u16::from_be_bytes([rest[0], rest[1]]),
    })
}

/// Builds a connect request — needed by the client (`client_connect`) and by tests.
pub fn encode_socks5_request(target: &Target, port: u16) -> Result<Vec<u8>> {
    let mut out = vec![VERSION, CMD_CONNECT, 0];
    match target {
        Target::Ip(ip) => {
            out.push(ATYP_IPV4);
            out.extend_from_slice(&ip.octets());
        }
        Target::Domain(domain) => {
            let bytes = domain.as_bytes();
            if bytes.is_empty() || bytes.len() > MAX_NAME_LEN {
                bail!("the domain name does not fit in a SOCKS5 frame: {} bytes", bytes.len());
            }
            out.push(ATYP_DOMAIN);
            out.push(bytes.len() as u8);
            out.extend_from_slice(bytes);
        }
    }
    out.extend_from_slice(&port.to_be_bytes());
    Ok(out)
}

/// Builds the server reply: the address in the reply is always zero, clients do not read it.
pub fn encode_socks5_reply(code: u8) -> [u8; 10] {
    [VERSION, code, 0, ATYP_IPV4, 0, 0, 0, 0, 0, 0]
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
