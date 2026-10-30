//! Where relay candidates for a dial come from.

use crate::transport::net::id::PeerId;
use crate::transport::net::stream::BoxFuture;
use std::net::SocketAddrV4;
use std::sync::Arc;
use townjump_dht::RelayAddress;

/// What a relay publishes about itself: the key of its node and, when it has one, the public address clients
/// reach it at. Without an address clients find the relay by its key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RelayAdvert {
    pub key: [u8; 32],
    pub address: Option<SocketAddrV4>,
}

impl From<RelayAddress> for RelayAdvert {
    fn from(a: RelayAddress) -> Self {
        Self {
            key: a.public_key,
            address: Some(a.addr),
        }
    }
}

/// A network member that runs a relay: the member, the key of its relay node and, when advertised, its address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RelayCandidate {
    pub member: PeerId,
    pub relay: [u8; 32],
    pub address: Option<SocketAddrV4>,
}

/// Candidates for a dial to the given peer, sorted by member. Empty — no relay is tried.
pub type RelaySource = Arc<dyn Fn(PeerId) -> BoxFuture<'static, Vec<RelayCandidate>> + Send + Sync>;
