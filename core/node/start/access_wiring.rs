//! The membership list at node start: the boot run (witnesses, migration of a long-time node) and the watcher.
//!
//! What it does: builds the boot input from the config and the address cache, runs the boot before the node
//! writes anything of its own to the shared replica, then starts the watcher that publishes the view.
//! What it doesn't do: no tally and no records of its own (`access`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::task::AbortHandle;

use crate::config::config_file::forget_invited_peers;
use crate::members::access::boot::{run_boot, AccessBoot};
use crate::members::access::runtime::Access;
use crate::members::access::watch::spawn_access;
use crate::members::membership::Members;
use crate::node::config::NodeConfig;
use crate::node::output::say;
use crate::proxy::domains::DomainList;
use crate::transport::net::PeerId;
use crate::transport::net_mainline::unix_now;
use crate::transport::peers_cache::PeersCache;

/// What `start_access` needs, collected before the address cache goes to the main loop.
pub(super) struct AccessStart {
    pub(super) boot: AccessBoot,
    pub(super) open_list: bool,
    pub(super) root: PeerId,
    pub(super) invited_file: Option<PathBuf>,
}

/// Ids from the invite (in the order given, no repeats).
fn initial_ids(cfg: &NodeConfig) -> Vec<PeerId> {
    let mut ids: Vec<PeerId> = Vec::new();
    for addr in &cfg.initial_peers {
        if !ids.contains(&addr.id) {
            ids.push(addr.id);
        }
    }
    ids
}

/// Ids from the address cache: the freshest first, equal times by id.
fn cached_ids(cache: &PeersCache) -> Vec<PeerId> {
    let mut seen: Vec<(PeerId, u64)> = cache.seen().into_iter().collect();
    seen.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    seen.into_iter().map(|(id, _)| id).collect()
}

pub(super) fn access_start(cfg: &NodeConfig, me: PeerId, cache: &PeersCache) -> AccessStart {
    let boot = AccessBoot {
        me,
        initial: initial_ids(cfg),
        cached: cached_ids(cache),
        now_unix: unix_now(),
        root: cfg.net_root,
    };
    AccessStart {
        boot,
        open_list: cfg.open_list,
        root: cfg.net_root,
        invited_file: cfg.invited_peers_file.clone(),
    }
}

/// Runs the boot and starts the watcher; with `open_list` only creates the `Access` (the view stays open).
pub(super) async fn start_access(
    list: &DomainList,
    members: &Members,
    st: AccessStart,
    handles: &mut Vec<AbortHandle>,
) -> Arc<Access> {
    if st.open_list {
        return Arc::new(Access::new(list, members, st.root));
    }
    boot_or_say(list, &st.boot, st.invited_file.as_deref()).await;
    let access = Arc::new(Access::new(list, members, st.root));
    handles.extend(spawn_access(access.clone()));
    access
}

/// A failed boot is logged and the node goes on: the watcher still publishes the view. A finished boot has
/// put the invited peers into the shared replica, which reaches the disk with a delay, so it is flushed first;
/// only then the invited peers file is no longer needed.
async fn boot_or_say(list: &DomainList, boot: &AccessBoot, invited_file: Option<&Path>) {
    let shared = list.shared_replica();
    let res = run_boot(shared.as_ref(), list.own_replica().as_ref(), boot).await;
    if let Err(e) = &res {
        say("access", format!("boot failed: {e}"));
    }
    let flush = match res {
        Ok(_) => shared.flush().await,
        Err(_) => Ok(()),
    };
    if let Err(e) = &flush {
        say(
            "access",
            format!("saving the list after the boot failed: {e}"),
        );
    }
    forget_after_boot(&saved(&res, flush), invited_file);
}

/// The boot result that also counts the flush: a boot that finished but was not saved counts as failed.
fn saved<E, F: std::fmt::Display>(
    res: &Result<usize, E>,
    flush: Result<(), F>,
) -> Result<usize, String> {
    match (res, flush) {
        (Ok(n), Ok(())) => Ok(*n),
        (Ok(_), Err(e)) => Err(format!("not saved: {e}")),
        (Err(_), _) => Err("boot failed".to_string()),
    }
}

/// Removes the invited peers file after a boot that finished; a failed boot keeps it for the next start.
fn forget_after_boot<E>(res: &Result<usize, E>, slot_file: Option<&Path>) {
    let Some(path) = slot_file.filter(|_| res.is_ok()) else {
        return;
    };
    forget_invited_peers(path);
}

#[cfg(test)]
#[path = "access_wiring_tests.rs"]
mod tests;
