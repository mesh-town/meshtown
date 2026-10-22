//! Node address: an id plus hints on how to reach it, and a link to the relay.
//! There is no parsing or printing of addresses here.

use crate::net::id::PeerId;
use std::net::SocketAddr;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RelayRef {
    Url(String),
    Peer(PeerId),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum AddrHint {
    Ip(SocketAddr),
    Relay(RelayRef),
    Opaque { kind: u64, bytes: Vec<u8> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeerAddr {
    pub id: PeerId,
    pub hints: Vec<AddrHint>,
}

impl PeerAddr {
    pub fn new(id: PeerId) -> Self {
        PeerAddr {
            id,
            hints: Vec::new(),
        }
    }

    /// Merges hints without repeats, own ones first; a foreign `id` does nothing.
    pub fn merge(&mut self, other: &PeerAddr) {
        if other.id != self.id {
            return;
        }
        for hint in &other.hints {
            if !self.hints.contains(hint) {
                self.hints.push(hint.clone());
            }
        }
    }
}

#[cfg(test)]
#[path = "addr_tests.rs"]
mod tests;
