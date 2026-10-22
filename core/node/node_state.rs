//! The "this node works as an exit" flag in the network's shared document, and shared helpers for reading/writing
//! a single value by key in the same document.
//!
//! What it does: announces whether the node's exit is on in the same iroh-docs document where
//! `list/` and `chk/` live (key `node/exit/`, own author), at start and on every change of
//! `exit.enabled`; reads the flag of other members from there. Provides the shared `read_values`/`own_value`
//! for modules that publish their own value under a separate key the same way — the first
//! such module is `geoip::board`, the next is the `live-node-name` task.
//! What it doesn't do: does not store the flag on disk, does not check that the exit is reachable — it only
//! publishes a switch whose source of truth is `RunningNode.exit_on`.
//! Links: `node::start_node` starts `run_exit_announce` as a background task and wakes it from
//! `RunningNode::set_exit_enabled` through `Notify`; `node_ctl::snapshot::collect` calls
//! `read_exit_flags` to give the snapshot's rows of others the `exit` flag.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::transport::net::{live, NetError, PeerId, Replica};
use anyhow::Result;
use bytes::Bytes;
use tokio::sync::Notify;
use tokio::time::timeout;

use crate::node::output::say;

/// Key of the exit flag in the network's shared document.
pub const EXIT_KEY: &[u8] = b"node/exit/";
/// Value of the record when the exit is on; off is a tombstone.
pub const EXIT_ON: &[u8] = b"on";
/// How often to repeat the announcement after a failed write.
const RETRY_AFTER: Duration = Duration::from_secs(30);

/// Announces this node's exit flag: `on` is a non-empty record, otherwise a tombstone.
pub async fn announce_exit(r: &dyn Replica, on: bool) -> Result<()> {
    if on {
        r.set_own(EXIT_KEY, Bytes::from_static(EXIT_ON))
            .await
            .map_err(NetError::into_anyhow)?;
    } else {
        r.del_own(EXIT_KEY).await.map_err(NetError::into_anyhow)?;
    }
    Ok(())
}

/// The exit flag of each author who has a record under the key; a tombstone is `false`,
/// no record from an author — the author does not appear in the map.
pub async fn read_exit_flags(r: &dyn Replica) -> Result<HashMap<PeerId, bool>> {
    let entries = r.scan(EXIT_KEY).await.map_err(NetError::into_anyhow)?;
    Ok(entries
        .into_iter()
        .filter(|e| e.key == EXIT_KEY)
        .map(|e| (e.author, !e.is_tombstone()))
        .collect())
}

/// The value of the key per author who has one: tombstones are excluded.
pub async fn read_values(r: &dyn Replica, key: &[u8]) -> Result<HashMap<PeerId, Bytes>> {
    let entries = r.scan(key).await.map_err(NetError::into_anyhow)?;
    Ok(live(entries)
        .into_iter()
        .filter(|e| e.key == key)
        .map(|e| (e.author, e.value))
        .collect())
}

/// Own value of the key and the write time in microseconds; a tombstone or no record is
/// `None`.
pub async fn own_value(r: &dyn Replica, key: &[u8]) -> Result<Option<(Bytes, u64)>> {
    let entry = r.get(r.me(), key).await.map_err(NetError::into_anyhow)?;
    Ok(entry
        .filter(|e| !e.is_tombstone())
        .map(|e| (e.value, e.ts_micros)))
}

/// Keeps the exit flag announced: writes at start and on every alarm from
/// `set_exit_enabled`, repeats on a timer if a write failed.
pub async fn run_exit_announce(
    r: Arc<dyn Replica>,
    exit_on: Arc<AtomicBool>,
    changed: Arc<Notify>,
) {
    let mut last_announced: Option<bool> = None;
    loop {
        let on = exit_on.load(Ordering::Relaxed);
        if last_announced != Some(on) {
            match announce_exit(r.as_ref(), on).await {
                Ok(()) => {
                    last_announced = Some(on);
                    say(
                        "exit",
                        format!("announced to peers: exit {}", if on { "on" } else { "off" }),
                    );
                }
                Err(e) => say("exit", format!("exit announcement not written: {e:#}")),
            }
        }
        let _ = timeout(RETRY_AFTER, changed.notified()).await;
    }
}

#[cfg(test)]
#[path = "node_state_tests.rs"]
mod tests;
