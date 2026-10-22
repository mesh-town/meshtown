//! Membership of a broadcast topic: peers and events. The replica is in `replica`, the sync in `sync_backend`.
//!
//! `Membership` is the peers of a broadcast topic (gossip), not the `Members` (handshaken
//! connections of a node): the first are seen from topic events, the second from verified connections.

use crate::transport::net::error::NetResult;
use crate::transport::net::id::PeerId;
use crate::transport::net::stream::BoxFuture;
use bytes::Bytes;
use tokio::sync::broadcast;

/// Topic event: a peer appeared or left, a message arrived, a subscriber fell behind, the topic closed.
#[derive(Clone, Debug)]
pub enum SwarmEvent {
    NeighborUp(PeerId),
    NeighborDown(PeerId),
    Received {
        from: PeerId,
        msg: Bytes,
    },
    /// The receiver fell behind, some events were lost.
    Lagged,
    /// The topic closed with a receive error; the error text is for the log.
    Closed(String),
}

pub trait Membership: Send + Sync {
    /// Invites peers into the topic; an empty list gives `Ok` without touching the network.
    fn join(&self, peers: &[PeerId]) -> BoxFuture<'_, NetResult<()>>;
    /// Broadcasts a message to peers; with no peers — `Ok`.
    fn broadcast(&self, msg: Bytes) -> BoxFuture<'_, NetResult<()>>;
    /// New events since the call.
    fn subscribe(&self) -> broadcast::Receiver<SwarmEvent>;
}
