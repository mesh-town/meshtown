//! Node name in the network's shared document — the key `node/name/`, each author has its own record.
//!
//! What it does: writes the effective name of this node (`config.node.name` or the default name) and on
//! every name change rewrites the record; reads the peers' names and puts them into `Members` —
//! so the name of a node with no direct connection, and a name change without reconnecting, reach
//! the page, `peer_label` and `ports`. The name from the handshake stays: it is faster, the document is more accurate.
//! What it doesn't do: it does not write foreign names to disk (the document replica is on disk anyway), does not keep
//! tombstones (an empty or unfit name is replaced by the default name), does not touch the snapshot.
//! Links: `node::start::serving::setup_serving` calls `spawn_node_name`;
//! `node::running::RunningNode::set_node_name` sends the new name into the channel `MemberContext.own_name_tx`;
//! `node::start::boot::pick_node_name` takes `effective_node_name`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::transport::net::{NetError, PeerId, Replica};
use anyhow::Result;
use bytes::Bytes;
use tokio::sync::watch;
use tokio::task::AbortHandle;
use tokio::time::{sleep, timeout};

use crate::members::membership::{MemberContext, Members};
use crate::members::words::{default_node_name, trim_and_validate_node_name, MAX_NAME_BYTES};
use crate::node::node_state::{own_value, read_values};
use crate::node::output::{say, short_peer_id};
use crate::proxy::domains::DomainList;

/// Key of the node name in the network's shared document.
pub const NODE_NAME_KEY: &[u8] = b"node/name/";

/// Polling period of the document by the watcher.
const WATCH_PERIOD: Duration = Duration::from_secs(2);

/// Pause before retrying a write if it failed.
const ANNOUNCE_RETRY: Duration = Duration::from_secs(30);

/// Effective name: the trimmed configured one if it passes `trim_and_validate_node_name`, otherwise the default
/// name from the key.
pub fn effective_node_name(configured: Option<&str>, id: PeerId) -> String {
    match configured.map(trim_and_validate_node_name) {
        Some(Ok(name)) => name.to_string(),
        _ => default_node_name(id.as_bytes()),
    }
}

/// A fit name from the record bytes: not longer than `MAX_NAME_BYTES` (before trimming), UTF-8, passes
/// `trim_and_validate_node_name`; otherwise `None`. Does not panic on any bytes.
pub fn parse_node_name(bytes: &[u8]) -> Option<String> {
    let fits = bytes.len() <= MAX_NAME_BYTES;
    let text = std::str::from_utf8(bytes).ok().filter(|_| fits)?;
    trim_and_validate_node_name(text).ok().map(str::to_string)
}

/// Writes the name as its own record only if its own record differs: `Ok(true)` — written.
pub async fn ensure_node_name(r: &dyn Replica, name: &str) -> Result<bool> {
    let own = own_value(r, NODE_NAME_KEY).await?;
    let same = own.is_some_and(|(bytes, _)| bytes.as_ref() == name.as_bytes());
    if same {
        return Ok(false);
    }
    r.set_own(NODE_NAME_KEY, Bytes::copy_from_slice(name.as_bytes()))
        .await
        .map_err(NetError::into_anyhow)?;
    Ok(true)
}

/// The name of each author whose value is fit; the others do not appear in the map.
pub async fn read_node_names(r: &dyn Replica) -> Result<HashMap<PeerId, String>> {
    let values = read_values(r, NODE_NAME_KEY).await?;
    Ok(values
        .into_iter()
        .filter_map(|(id, bytes)| parse_node_name(&bytes).map(|n| (id, n)))
        .collect())
}

/// What to put into `Members` from a fresh read of the document. `seen` is the last read name
/// per author. Own id is skipped; the same name as in `seen` — nothing; a changed or
/// first-appearing one — into the answer (the first appearance is always applied: the document converges to
/// the truth); an author who vanished from the read touches neither `seen` nor `Members`.
pub(crate) fn names_to_apply(
    seen: &mut HashMap<PeerId, String>,
    read: HashMap<PeerId, String>,
    me: PeerId,
    _members: &Members,
) -> Vec<(PeerId, String)> {
    let mut out = Vec::new();
    for (id, name) in read {
        let fresh = id != me && seen.get(&id) != Some(&name);
        if fresh {
            seen.insert(id, name.clone());
            out.push((id, name));
        }
    }
    out
}

/// One publisher pass: the name written from the channel; `true` — written or already matched.
async fn announce_once(r: &dyn Replica, name: &str) -> bool {
    match ensure_node_name(r, name).await {
        Ok(written) => {
            written.then(|| {
                say(
                    "docs",
                    format!("name \"{name}\" written to the network document"),
                )
            });
            true
        }
        Err(e) => {
            say(
                "docs",
                format!("name not written to the network document: {e:#}"),
            );
            false
        }
    }
}

/// Publisher: keeps its record equal to the effective name — writes at start and on every
/// channel change; after an error retries in 30 s. A closed channel — exit.
pub async fn run_name_announce(r: Arc<dyn Replica>, mut name: watch::Receiver<String>) {
    loop {
        let current = name.borrow_and_update().clone();
        let ok = announce_once(r.as_ref(), &current).await;
        let woke = match ok {
            true => name.changed().await.is_ok(),
            false => timeout(ANNOUNCE_RETRY, name.changed())
                .await
                .map_or(true, |r| r.is_ok()),
        };
        if !woke {
            return;
        }
    }
}

/// Log line about a name change of a member that was read before.
fn report_rename(id: PeerId, old: Option<String>, new: &str) {
    let changed = old.filter(|o| o != new);
    if let Some(old) = changed {
        say(
            "member",
            format!("{} changed name: \"{old}\" → \"{new}\"", short_peer_id(&id)),
        );
    }
}

/// Watcher: once per `WATCH_PERIOD` reads the document and puts names into `Members`; a read error is
/// a quiet skip of the tick. An author's first appearance is written silently, a change — as a log line.
pub async fn run_name_watch(r: Arc<dyn Replica>, me: PeerId, members: Members) {
    let mut seen: HashMap<PeerId, String> = HashMap::new();
    loop {
        sleep(WATCH_PERIOD).await;
        let Ok(read) = read_node_names(r.as_ref()).await else {
            continue;
        };
        let known: Vec<PeerId> = read
            .keys()
            .filter(|id| seen.contains_key(*id))
            .copied()
            .collect();
        for (id, name) in names_to_apply(&mut seen, read, me, &members) {
            let old = known.contains(&id).then(|| members.name_of(id)).flatten();
            members.set_name(id, name.clone());
            report_rename(id, old, &name);
        }
    }
}

/// Starts the publisher and the watcher as background tasks; puts the handles into `boot.tasks`.
pub fn spawn_node_name(list: &DomainList, ctx: &MemberContext) -> [AbortHandle; 2] {
    let announce = tokio::spawn(run_name_announce(
        list.shared_replica(),
        ctx.own_name_tx.subscribe(),
    ));
    let watch = tokio::spawn(run_name_watch(
        list.shared_replica(),
        ctx.own_id,
        ctx.members.clone(),
    ));
    [announce.abort_handle(), watch.abort_handle()]
}

#[cfg(test)]
#[path = "node_name_tests.rs"]
mod tests;
