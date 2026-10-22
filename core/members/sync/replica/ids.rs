//! Replica keys: `"net"` — from the network seed, `"domains"` — permanent, not from the node's secret.

use crate::net::replica::ReplicaId;

/// The id of the `"net"` replica for the given network seed.
pub(crate) fn net_id(seed: &[u8; 32]) -> ReplicaId {
    ReplicaId(blake3::derive_key("meshtown replica net v1", seed))
}

/// The id of the `"domains"` replica: one per node, changing the secret does not orphan the list.
pub(crate) fn domains_id() -> ReplicaId {
    ReplicaId(blake3::derive_key("meshtown replica domains v1", &[]))
}

/// The topic of the replica's live broadcast: derived from the id, a foreign network's topic will not match.
pub(crate) fn topic_of(id: &ReplicaId) -> [u8; 32] {
    blake3::derive_key("meshtown replica topic v1", id.as_bytes())
}

#[cfg(test)]
#[path = "ids_tests.rs"]
mod tests;
