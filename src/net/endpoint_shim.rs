//! Tests only: a builder of a node address with hints — `EndpointAddr`, `TransportAddr`, `RelayUrl` —
//! on top of `PeerAddr`. Old format tests (invite, address cache, DHT record) build an address
//! in this form; the hint order is the same as it was: relay first, then IPs ascending.

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::str::FromStr;

use super::{AddrHint, PeerAddr, PeerId, RelayRef};

/// Error parsing a relay server address.
#[derive(Debug)]
pub struct BadRelayUrl;

/// Relay server address; an empty path is extended to `/`, as in real URL parsing.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RelayUrl(String);

impl FromStr for RelayUrl {
    type Err = BadRelayUrl;
    fn from_str(s: &str) -> Result<Self, BadRelayUrl> {
        let rest = s.strip_prefix("https://").or_else(|| s.strip_prefix("http://")).ok_or(BadRelayUrl)?;
        Ok(RelayUrl(if rest.contains('/') { s.to_string() } else { format!("{s}/") }))
    }
}

impl std::fmt::Display for RelayUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Address hint; the variant order sets the order in `EndpointAddr`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransportAddr {
    Relay(RelayUrl),
    Ip(SocketAddr),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndpointAddr {
    pub id: PeerId,
    pub addrs: BTreeSet<TransportAddr>,
}

impl EndpointAddr {
    pub fn new(id: PeerId) -> Self {
        EndpointAddr { id, addrs: BTreeSet::new() }
    }

    pub fn from_parts(id: PeerId, addrs: impl IntoIterator<Item = TransportAddr>) -> Self {
        EndpointAddr { id, addrs: addrs.into_iter().collect() }
    }

    pub fn with_ip_addr(mut self, a: SocketAddr) -> Self {
        self.addrs.insert(TransportAddr::Ip(a));
        self
    }

    pub fn with_relay_url(mut self, u: RelayUrl) -> Self {
        self.addrs.insert(TransportAddr::Relay(u));
        self
    }

    pub fn relay_urls(&self) -> impl Iterator<Item = &RelayUrl> {
        self.addrs.iter().filter_map(|a| match a {
            TransportAddr::Relay(u) => Some(u),
            TransportAddr::Ip(_) => None,
        })
    }

    pub fn ip_addrs(&self) -> impl Iterator<Item = &SocketAddr> {
        self.addrs.iter().filter_map(|a| match a {
            TransportAddr::Ip(s) => Some(s),
            TransportAddr::Relay(_) => None,
        })
    }
}

pub fn from_endpoint_addr(addr: &EndpointAddr) -> PeerAddr {
    let hints = addr.addrs.iter().map(|a| match a {
        TransportAddr::Ip(sa) => AddrHint::Ip(*sa),
        TransportAddr::Relay(u) => AddrHint::Relay(RelayRef::Url(u.to_string())),
    });
    PeerAddr { id: addr.id, hints: hints.collect() }
}

pub fn to_endpoint_addr(addr: &PeerAddr) -> EndpointAddr {
    let parts = addr.hints.iter().filter_map(|h| match h {
        AddrHint::Ip(sa) => Some(TransportAddr::Ip(*sa)),
        AddrHint::Relay(RelayRef::Url(u)) => u.parse().ok().map(TransportAddr::Relay),
        _ => None,
    });
    EndpointAddr::from_parts(addr.id, parts)
}
