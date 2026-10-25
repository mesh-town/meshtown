//! The address this node gives out for direct dials.

use crate::transport::net::addr::AddrHint;
use std::net::SocketAddr;
use townjump_dht::NodeMode;

/// The DHT address of the node when it is a persistent one and reachable from outside; otherwise nothing.
pub(super) fn own_hints(mode: &NodeMode) -> Vec<AddrHint> {
    match (mode.persistent && !mode.firewalled, mode.address) {
        (true, Some(address)) => vec![AddrHint::Ip(SocketAddr::V4(address))],
        _ => Vec::new(),
    }
}

#[cfg(test)]
#[path = "own_addr_tests.rs"]
mod tests;
