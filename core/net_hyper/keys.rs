//! Node keys for townjump: one seed — one identity in the mesh and in the DHT.
//! The ed25519-dalek 2 (townjump) and 3 (mesh) types do not mix: only `[u8; 32]` crosses the boundary.

use crate::net::error::{NetError, NetResult};
use crate::net::id::{NodeSecret, PeerId};
use townjump_keys::KeyPair;

/// Townjump key pair from the node secret: the public key matches `secret.peer_id()`.
pub fn key_pair(secret: &NodeSecret) -> KeyPair {
    townjump_keys::key_pair_from_seed(&secret.to_bytes())
}

/// Identity of a peer from the public key in the DHT; not a curve point — `NetError::Other`.
pub fn peer_of(key: &[u8; 32]) -> NetResult<PeerId> {
    PeerId::from_bytes(key).map_err(|e| NetError::Other(anyhow::anyhow!("{e}")))
}

#[cfg(test)]
#[path = "keys_tests.rs"]
mod tests;
