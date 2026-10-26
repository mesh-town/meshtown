//! Announcer: keeps our own publications written in the shared document.
//!
//! What it does: at startup removes our own entries that are not in the table; on each change of the
//! table writes only the difference from the last successfully announced state and checks the ports
//! (`connect_loopback_port`); on a document error it retries after 30 s.
//! What it does not do: it does not accept connections (that is `server`).
//! Links: `PublishedPortsTable` (the list, the signal), `PortsDoc` (the document).

use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use tokio::time::timeout;

use super::record::PortRecord;
use super::server::connect_loopback_port;
use super::store::PortsDoc;
use super::table::{LocalState, PublishedPortsTable};
use super::types::{Proto, Published};
use crate::node::output::say;

/// Retry after a document error and the ceiling of waiting for the signal.
const RETRY_AFTER: Duration = Duration::from_secs(30);

/// What to write and what to remove: the difference between the list and the last announced one.
pub(crate) fn diff_published(
    published: &[Published],
    announced: &HashMap<(Proto, u16), PortRecord>,
) -> (Vec<Published>, Vec<(Proto, u16)>) {
    let write = published
        .iter()
        .filter(|p| announced.get(&(p.proto, p.port)) != Some(&PortRecord::from(*p)))
        .cloned()
        .collect();
    let mut remove: Vec<(Proto, u16)> = announced
        .keys()
        .copied()
        .filter(|key| published.iter().all(|p| (p.proto, p.port) != *key))
        .collect();
    remove.sort_unstable();
    (write, remove)
}

/// The announcer's endless loop.
pub async fn run_announce(store: PortsDoc, table: PublishedPortsTable) {
    let changed = table.changed_signal();
    let mut announced: HashMap<(Proto, u16), PortRecord> = HashMap::new();
    let mut has_cleaned_stale = false;
    loop {
        if let Err(e) = cycle(&store, &table, &mut announced, &mut has_cleaned_stale).await {
            say("port", format!("publications not written: {e:#}"));
        }
        let _ = timeout(RETRY_AFTER, changed.notified()).await;
    }
}

async fn cycle(
    store: &PortsDoc,
    table: &PublishedPortsTable,
    announced: &mut HashMap<(Proto, u16), PortRecord>,
    has_cleaned_stale: &mut bool,
) -> Result<()> {
    let published: Vec<Published> = table
        .published_with_state()
        .into_iter()
        .map(|(p, _)| p)
        .collect();
    if !*has_cleaned_stale {
        remove_unlisted_records(store, &published).await?;
        *has_cleaned_stale = true;
    }
    let (write, remove) = diff_published(&published, announced);
    for p in write {
        if p.proto.is_tcp() {
            check_local_port(table, &p).await;
        }
        store.publish(&p).await?;
        announced.insert((p.proto, p.port), PortRecord::from(&p));
    }
    for (proto, port) in remove {
        store.unpublish(proto, port).await?;
        announced.remove(&(proto, port));
    }
    Ok(())
}

/// Checks a TCP port of this device and records the result; a UDP port cannot be checked.
async fn check_local_port(table: &PublishedPortsTable, p: &Published) {
    let local_state = match connect_loopback_port(p.port).await {
        Some(_) => LocalState::Ok,
        None => LocalState::Refused,
    };
    table.set_local_state(p.proto, p.port, local_state);
}

/// Removes our own live entries that are not in the table.
async fn remove_unlisted_records(store: &PortsDoc, published: &[Published]) -> Result<()> {
    for (proto, port) in store.own_ports().await? {
        if published.iter().all(|p| (p.proto, p.port) != (proto, port)) {
            store.unpublish(proto, port).await?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "announce_tests.rs"]
mod tests;
