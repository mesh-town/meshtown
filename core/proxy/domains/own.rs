//! The node's own domain list: a closed replica without synchronization.
//!
//! What it does: reads the domain list from a replica (`Replica`).
//! What it does not do: it knows nothing about how the replica is stored or synced (the space and secret are derived by `sync::replica`),
//! does not start synchronization, does not change or clean the shared replica.

use anyhow::Result;

use super::{domain_from_key, LIST_PREFIX};
use crate::transport::net::{live, NetError, Replica};

/// The replica's domains: the last entry per key among all authors, `list/`, alphabetical;
/// domains with a tombstone in the last entry are not included.
pub(crate) async fn read_domain_list(replica: &dyn Replica) -> Result<Vec<String>> {
    let entries = replica
        .latest_per_key(LIST_PREFIX.as_bytes())
        .await
        .map_err(NetError::into_anyhow)?;
    let mut out: Vec<String> = live(entries)
        .iter()
        .filter_map(|e| domain_from_key(&e.key))
        .collect();
    out.sort();
    out.dedup();
    Ok(out)
}
