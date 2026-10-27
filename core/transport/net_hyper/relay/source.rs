//! Where relay candidates for a dial come from.

use crate::transport::net::id::PeerId;
use crate::transport::net::stream::BoxFuture;
use std::sync::Arc;

/// A network member that runs a relay: the member and the key of its relay node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RelayCandidate {
    pub member: PeerId,
    pub relay: [u8; 32],
}

/// Candidates for a dial to the given peer, sorted by member. Empty — no relay is tried.
pub type RelaySource = Arc<dyn Fn(PeerId) -> BoxFuture<'static, Vec<RelayCandidate>> + Send + Sync>;
