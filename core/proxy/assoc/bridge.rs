//! Conversion between a SOCKS5 UDP datagram and a udpgw frame. Pure functions, no network.

use std::net::{IpAddr, SocketAddr};

use crate::exit::udpgw_codec::{self as codec, Frame};
use crate::proxy::socks5::{Target, UdpPacket};

/// The connection id of the gateway session for a destination: one per address family, because the exit binds
/// the socket of a session to the family of its first destination.
pub(super) fn conn_id_for(target: &codec::Target) -> u16 {
    u16::from(matches!(target, codec::Target::Ip(SocketAddr::V6(_))))
}

/// The gateway form of a destination: IP literals are canonical (`::ffff:a.b.c.d` is IPv4), a domain that is
/// an IP literal becomes an IP.
fn gateway_target(target: Target, port: u16) -> codec::Target {
    let text = target.to_string();
    match text.parse::<IpAddr>() {
        Ok(ip) => codec::Target::Ip(SocketAddr::new(ip.to_canonical(), port)),
        Err(_) => codec::Target::Domain(text, port),
    }
}

/// The wire bytes of a DATA frame for a client datagram; `None` if it does not fit one frame.
pub(super) fn to_gateway_frame(packet: UdpPacket) -> Option<Vec<u8>> {
    let target = gateway_target(packet.target, packet.port);
    let conn_id = conn_id_for(&target);
    let frame = Frame {
        flags: codec::FLAG_DATA,
        conn_id,
        target: Some(target),
        data: packet.data,
    };
    codec::encode(&frame)
}

/// The source and payload of a reply frame; `None` for anything that is not a DATA frame from an address.
pub(super) fn from_gateway_frame(frame: Frame) -> Option<(SocketAddr, Vec<u8>)> {
    let is_data = frame.flags & codec::FLAG_DATA != 0 && frame.flags & codec::FLAG_ERR == 0;
    match frame.target {
        Some(codec::Target::Ip(from)) if is_data => Some((from, frame.data)),
        _ => None,
    }
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod tests;
