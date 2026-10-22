//! SOCKS5 frames (RFC 1928): codes, parsing the client's request and building the reply. Pure functions —
//! no network, they are checked by `frame_tests.rs`.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddrV4};

use anyhow::{bail, Result};

/// The protocol version.
pub const VERSION: u8 = 5;
/// The "no authentication" method.
pub const METHOD_NO_AUTH: u8 = 0x00;
pub const METHOD_NO_ACCEPTABLE: u8 = 0xFF;
/// The "connect" command.
pub const CMD_CONNECT: u8 = 1;
/// The "UDP associate" command.
pub const CMD_UDP_ASSOCIATE: u8 = 3;
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
    Ip(IpAddr),
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
    /// A target from a text node name (HTTP proxy): an IP literal (IPv4, or IPv6 without brackets) is `Ip`,
    /// a name is `Domain` in lower case. Empty, longer than `MAX_NAME_LEN`, bracketed, or with a `:` that is
    /// not an IPv6 literal is `None`.
    pub fn from_host(host: &str) -> Option<Target> {
        if host.is_empty() || host.len() > MAX_NAME_LEN || host.starts_with('[') {
            return None;
        }
        if host.contains(':') {
            return host
                .parse::<Ipv6Addr>()
                .ok()
                .map(|ip| Target::Ip(IpAddr::V6(ip)));
        }
        Some(match host.parse::<Ipv4Addr>() {
            Ok(ip) => Target::Ip(IpAddr::V4(ip)),
            Err(_) => Target::Domain(host.to_lowercase()),
        })
    }
}

/// The command of a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Connect,
    UdpAssociate,
}

/// A parsed request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub command: Command,
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
    /// The command is neither `CONNECT` nor `UDP ASSOCIATE`.
    Command(u8),
    /// The address type is not supported.
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
            RequestError::Command(c) => write!(
                f,
                "command {c}: only CONNECT and UDP ASSOCIATE are supported"
            ),
            RequestError::AddressKind(a) => write!(f, "address type {a} is not supported"),
            RequestError::EmptyName => write!(f, "empty domain name"),
            RequestError::BadName => write!(f, "domain name is not UTF-8"),
        }
    }
}

impl std::error::Error for RequestError {}

/// Parses a request. A pure function: there is no network inside.
pub fn parse_socks5_request(buf: &[u8]) -> std::result::Result<Request, RequestError> {
    if buf.len() < 4 {
        return Err(RequestError::Short);
    }
    if buf[0] != VERSION {
        return Err(RequestError::Version(buf[0]));
    }
    let command = match buf[1] {
        CMD_CONNECT => Command::Connect,
        CMD_UDP_ASSOCIATE => Command::UdpAssociate,
        other => return Err(RequestError::Command(other)),
    };
    let (target, port, _) = parse_address(&buf[3..])?;
    Ok(Request {
        command,
        target,
        port,
    })
}

/// Parses `ATYP ADDR PORT` at the start of `buf`: the target, the port and the number of bytes used.
pub(super) fn parse_address(buf: &[u8]) -> std::result::Result<(Target, u16, usize), RequestError> {
    let Some(&kind) = buf.first() else {
        return Err(RequestError::Short);
    };
    let (target, end) = match kind {
        ATYP_IPV4 => (Target::Ip(IpAddr::from(fixed::<4>(buf)?)), 5),
        ATYP_IPV6 => (Target::Ip(IpAddr::from(fixed::<16>(buf)?)), 17),
        ATYP_DOMAIN => domain_name(buf)?,
        other => return Err(RequestError::AddressKind(other)),
    };
    let port = buf.get(end..end + 2).ok_or(RequestError::Short)?;
    Ok((target, u16::from_be_bytes([port[0], port[1]]), end + 2))
}

/// A fixed-size address after the type byte.
fn fixed<const N: usize>(buf: &[u8]) -> std::result::Result<[u8; N], RequestError> {
    buf.get(1..1 + N)
        .and_then(|b| b.try_into().ok())
        .ok_or(RequestError::Short)
}

/// A length-prefixed domain name after the type byte; the offset past it.
fn domain_name(buf: &[u8]) -> std::result::Result<(Target, usize), RequestError> {
    let len = usize::from(*buf.get(1).ok_or(RequestError::Short)?);
    if len == 0 {
        return Err(RequestError::EmptyName);
    }
    let raw = buf.get(2..2 + len).ok_or(RequestError::Short)?;
    let name = std::str::from_utf8(raw).map_err(|_| RequestError::BadName)?;
    Ok((Target::Domain(name.to_ascii_lowercase()), 2 + len))
}

/// Builds a connect request — needed by the client (`client_connect`) and by tests.
pub fn encode_socks5_request(target: &Target, port: u16) -> Result<Vec<u8>> {
    let mut out = vec![VERSION, CMD_CONNECT, 0];
    encode_address(&mut out, target, port)?;
    Ok(out)
}

/// Builds the `UDP ASSOCIATE` request with an unknown source address (`0.0.0.0:0`), as clients send it.
pub fn encode_udp_associate_request() -> Vec<u8> {
    vec![VERSION, CMD_UDP_ASSOCIATE, 0, ATYP_IPV4, 0, 0, 0, 0, 0, 0]
}

/// Appends `ATYP ADDR PORT`; a domain name must fit in one length byte and not be empty.
pub(super) fn encode_address(out: &mut Vec<u8>, target: &Target, port: u16) -> Result<()> {
    match target {
        Target::Ip(IpAddr::V4(ip)) => {
            out.push(ATYP_IPV4);
            out.extend_from_slice(&ip.octets());
        }
        Target::Ip(IpAddr::V6(ip)) => {
            out.push(ATYP_IPV6);
            out.extend_from_slice(&ip.octets());
        }
        Target::Domain(domain) => {
            let bytes = domain.as_bytes();
            if bytes.is_empty() || bytes.len() > MAX_NAME_LEN {
                bail!(
                    "the domain name does not fit in a SOCKS5 frame: {} bytes",
                    bytes.len()
                );
            }
            out.push(ATYP_DOMAIN);
            out.push(bytes.len() as u8);
            out.extend_from_slice(bytes);
        }
    }
    out.extend_from_slice(&port.to_be_bytes());
    Ok(())
}

/// Builds the server reply with a zero bound address: clients do not read it after `CONNECT`.
pub fn encode_socks5_reply(code: u8) -> [u8; 10] {
    encode_socks5_reply_bound(code, SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0))
}

/// Builds the server reply carrying the bound address (the UDP relay of `UDP ASSOCIATE`).
pub fn encode_socks5_reply_bound(code: u8, bound: SocketAddrV4) -> [u8; 10] {
    let [a, b, c, d] = bound.ip().octets();
    let [p, q] = bound.port().to_be_bytes();
    [VERSION, code, 0, ATYP_IPV4, a, b, c, d, p, q]
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
