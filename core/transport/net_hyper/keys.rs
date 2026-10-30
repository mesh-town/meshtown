//! Node keys for townjump: one seed — one identity in the mesh and in the DHT.
//! Only `[u8; 32]` seeds and keys cross the boundary: the key types of the two crates stay separate.

use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::id::{NodeSecret, PeerId};
use townjump_keys::KeyPair;

/// Townjump key pair from the node secret: the public key matches `secret.peer_id()`.
pub fn key_pair(secret: &NodeSecret) -> KeyPair {
    townjump_keys::key_pair_from_seed(&secret.to_bytes())
}

/// Context of the key derivation for the relay key pair.
const RELAY_KEY_CONTEXT: &str = "meshtown relay key v1";

/// Permanent relay key pair of a node: derived from the node secret, so it survives restarts and
/// differs from the node key.
pub fn relay_key_pair(secret: &NodeSecret) -> KeyPair {
    let seed = blake3::derive_key(RELAY_KEY_CONTEXT, &secret.to_bytes());
    townjump_keys::key_pair_from_seed(&seed)
}

/// Identity of a peer from the public key in the DHT; not a curve point — `NetError::Other`.
pub fn peer_of(key: &[u8; 32]) -> NetResult<PeerId> {
    PeerId::from_bytes(key).map_err(|e| NetError::Other(anyhow::anyhow!("{e}")))
}

#[cfg(test)]
#[path = "keys_tests.rs"]
mod tests;
