//! The node's own domain list: a closed replica without synchronization.
//!
//! What it does: reads the domain list from a replica (`Replica`) and once copies into its own replica
//! the former list from the shared replica (`list/` of the shared replica), setting a migration marker.
//! What it does not do: it knows nothing about `iroh-docs` (the space and secret are derived by `sync::replica`),
//! does not start synchronization, does not change or clean the shared replica.
//! Links: `DomainList::open` (`domains.rs`) calls `migrate_shared_list`.

use anyhow::Result;
use bytes::Bytes;

use super::{domain_from_key, entry_key, ENTRY_VALUE, LIST_PREFIX};
use crate::node::output::say;
use crate::transport::net::{live, NetError, Replica};

/// The marker "the list was migrated from the shared document" (the value is non-empty: an empty one is a tombstone).
pub(crate) const MIGRATED_KEY: &[u8] = b"meta/own-list-from-shared/";

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

/// Whether a migration marker from any author exists: the node's author may change (a new secret),
/// while the migration stays done.
pub(crate) async fn is_migrated(own: &dyn Replica) -> Result<bool> {
    let entries = own
        .latest_per_key(MIGRATED_KEY)
        .await
        .map_err(NetError::into_anyhow)?;
    Ok(live(entries).iter().any(|e| e.key == MIGRATED_KEY))
}

/// Copies the live domains of the shared replica into its own once; sets the marker last.
/// Returns the number of migrated domains, `0` if the marker is already set.
pub(crate) async fn migrate_shared_list(shared: &dyn Replica, own: &dyn Replica) -> Result<usize> {
    if is_migrated(own).await? {
        return Ok(0);
    }
    let domains = read_domain_list(shared).await?;
    let value = Bytes::from_static(ENTRY_VALUE);
    for d in &domains {
        own.set_own(&entry_key(d), value.clone())
            .await
            .map_err(NetError::into_anyhow)?;
    }
    own.set_own(MIGRATED_KEY, value)
        .await
        .map_err(NetError::into_anyhow)?;
    if !domains.is_empty() {
        say(
            "domains",
            format!(
                "own list: migrated {} from the shared one (space {})",
                domains.len(),
                own.id().fmt_short()
            ),
        );
    }
    Ok(domains.len())
}
