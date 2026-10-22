//! The UDP datagram header of SOCKS5 (RFC 1928, section 7): `RSV(2) FRAG ATYP ADDR PORT DATA`. Pure functions —
//! no network, they are checked by `udp_tests.rs`.

use std::fmt;
use std::net::SocketAddr;

use anyhow::Result;

use super::frame::{encode_address, parse_address, RequestError, Target};

/// A datagram from a client: where to send the payload and the payload itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UdpPacket {
    pub target: Target,
    pub port: u16,
    pub data: Vec<u8>,
}

/// Why a datagram from a client was not accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UdpHeaderError {
    /// Fewer bytes than the header requires.
    Short,
    /// The fragment number is not zero: fragmentation is not supported.
    Fragmented(u8),
    /// The address type is not supported.
    AddressKind(u8),
    /// The domain name length is zero.
    EmptyName,
    /// The domain name is not UTF-8.
    BadName,
}

impl fmt::Display for UdpHeaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UdpHeaderError::Short => write!(f, "datagram shorter than its header"),
            UdpHeaderError::Fragmented(n) => {
                write!(f, "fragment {n}: fragmentation is not supported")
            }
            UdpHeaderError::AddressKind(a) => write!(f, "address type {a} is not supported"),
            UdpHeaderError::EmptyName => write!(f, "empty domain name"),
            UdpHeaderError::BadName => write!(f, "domain name is not UTF-8"),
        }
    }
}

impl std::error::Error for UdpHeaderError {}

impl From<RequestError> for UdpHeaderError {
    fn from(e: RequestError) -> Self {
        match e {
            RequestError::AddressKind(a) => UdpHeaderError::AddressKind(a),
            RequestError::EmptyName => UdpHeaderError::EmptyName,
            RequestError::BadName => UdpHeaderError::BadName,
            _ => UdpHeaderError::Short,
        }
    }
}

/// Parses a datagram from a client. The reserved bytes are not checked.
pub fn parse_udp_datagram(buf: &[u8]) -> std::result::Result<UdpPacket, UdpHeaderError> {
    if buf.len() < 4 {
        return Err(UdpHeaderError::Short);
    }
    if buf[2] != 0 {
        return Err(UdpHeaderError::Fragmented(buf[2]));
    }
    let (target, port, used) = parse_address(&buf[3..])?;
    Ok(UdpPacket {
        target,
        port,
        data: buf[3 + used..].to_vec(),
    })
}

/// Builds the datagram for a client: `from` is the address the payload came from.
pub fn encode_udp_reply(from: SocketAddr, data: &[u8]) -> Vec<u8> {
    let mut out = vec![0, 0, 0];
    let target = Target::Ip(from.ip());
    // An IP address always fits in the header.
    encode_address(&mut out, &target, from.port()).expect("an IP address is always encodable");
    out.extend_from_slice(data);
    out
}

/// Builds a datagram the way a client does (tests, manual clients); fails on an empty or too long domain name.
pub fn encode_udp_request(target: &Target, port: u16, data: &[u8]) -> Result<Vec<u8>> {
    let mut out = vec![0, 0, 0];
    encode_address(&mut out, target, port)?;
    out.extend_from_slice(data);
    Ok(out)
}

#[cfg(test)]
#[path = "udp_tests.rs"]
mod tests;
