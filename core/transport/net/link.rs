//! Information about the path to a node and about the node's own network (for display and choice).
//! Only data here, no behavior.

use crate::transport::net::addr::{AddrHint, RelayRef};
use std::net::SocketAddr;
use std::time::Duration;

#[derive(Clone, Debug, Default)]
pub enum PathKind {
    Direct,
    Relayed(RelayRef),
    #[default]
    Unknown,
}

#[derive(Clone, Debug, Default)]
pub struct LinkInfo {
    pub path: PathKind,
    pub remote: Option<SocketAddr>,
    pub rtt: Option<Duration>,
    pub sent_bytes: u64,
    pub recv_bytes: u64,
    /// All paths of the connection, not only the chosen one. Default 0 — mem, stubs.
    pub path_count: usize,
    /// STREAM frames sent since the start of the connection. Default 0.
    pub stream_frames: u64,
}

impl LinkInfo {
    /// Address hint for the chosen path: direct with a known address — `Ip`, through a relay — `Relay`.
    pub fn hint(&self) -> Option<AddrHint> {
        if let PathKind::Relayed(r) = &self.path {
            return Some(AddrHint::Relay(r.clone()));
        }
        match (&self.path, self.remote) {
            (PathKind::Direct, Some(sa)) => Some(AddrHint::Ip(sa)),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct NetInfo {
    /// "Network is visible" (UDP v4/v6 or a relay, no captive portal); `None` — unknown.
    pub net_seen: Option<bool>,
}

/// Change of the connection's chosen path.
#[derive(Clone, Debug)]
pub struct PathChange {
    pub path: PathKind,
    pub rtt: Option<Duration>,
    pub log_line: String,
}

#[cfg(test)]
#[path = "link_tests.rs"]
mod tests;
