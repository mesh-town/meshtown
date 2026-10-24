//! The slot watcher for a network closed by an administrator: when the slot's channel carries a new
//! `Closed`, the slot leaves its network the way a manual Leave does and the host notes the deletion.
//!
//! What it does: one watcher per slot cycle; it fires on a change of the value, not on every wake.
//! What it doesn't do: no counting of the close (access does), no answer to the page.

use std::sync::{Arc, Mutex, Weak};

use tokio::sync::watch;

use super::slots::{HostSlots, Slot};
use crate::config::config_file::NetSlotId;
use crate::members::access::tally::Closed;
use crate::node::node_ctl::NodeCtl;
use crate::node::output::say;
use crate::transport::net::PeerId;
use crate::web_gui::{HostControl, HostError};

/// What the host keeps about a network deleted by an administrator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DeletedNet {
    pub(super) label: Option<String>,
    pub(super) id_hex: Option<String>,
    pub(super) by: PeerId,
    pub(super) by_name: Option<String>,
    pub(super) at_unix: u64,
}

/// How a firing ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fired {
    Left,
    AlreadyGone,
    Stuck,
    Stopping,
}

/// Whether `seen` is a close the watcher has not acted on yet.
fn should_fire(seen: Option<&Closed>, last: Option<&Closed>) -> bool {
    seen.is_some() && seen != last
}

/// The host's record of deleted networks; one line per deletion.
#[derive(Default)]
pub(super) struct DeletedLog(Mutex<Vec<DeletedNet>>);

impl DeletedLog {
    fn push(&self, note: DeletedNet) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).push(note);
    }

    #[cfg(test)]
    pub(super) fn list(&self) -> Vec<DeletedNet> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

impl HostSlots {
    /// Starts the slot's watcher, replacing the previous one. Needs the slot's `ended`; a slot without
    /// it, or a host that is not behind an `Arc`, gets none.
    pub(super) fn watch_slot(&self, slot: &mut Slot) {
        if let Some(old) = slot.watch.take() {
            old.abort();
        }
        let Some(ended) = slot.ended.clone() else {
            return;
        };
        let task = crate::node::output::spawn(watch_closed(
            self.me.clone(),
            slot.handle.id.clone(),
            slot.ctl.clone(),
            ended,
        ));
        slot.watch = Some(task.abort_handle());
    }

    /// Records a deleted network for the page. The single hook of the deletion notice.
    pub(super) fn push_deleted(&self, note: DeletedNet) {
        self.deleted.push(note);
    }
}

async fn watch_closed(
    host: Weak<HostSlots>,
    id: NetSlotId,
    ctl: Arc<NodeCtl>,
    mut ended: watch::Receiver<bool>,
) {
    let mut closed = ctl.closed();
    let mut last: Option<Closed> = None;
    loop {
        let seen = closed.borrow_and_update().clone();
        if should_fire(seen.as_ref(), last.as_ref()) {
            let Some(host) = host.upgrade() else {
                return;
            };
            if let Some(c) = &seen {
                let fired = fire(&host, &id, &ctl, c);
                if fired == Fired::Left && !id.carries_process() {
                    return;
                }
            }
        }
        last = seen.or(last);
        tokio::select! {
            changed = closed.changed() => if changed.is_err() { return },
            done = ended.wait_for(|d| *d) => { let _ = done; return },
        }
    }
}

/// Leaves the network of the slot because `c` closed it; notes the deletion when the node left or could not.
fn fire(host: &HostSlots, id: &NetSlotId, ctl: &NodeCtl, c: &Closed) -> Fired {
    let (label, id_hex) = names_of(host, id);
    let by_name = ctl.peer_name(&c.by);
    let fired = leave_after_close(host, id, ctl, label.as_deref());
    if matches!(fired, Fired::Left | Fired::Stuck) {
        host.push_deleted(DeletedNet {
            label,
            id_hex,
            by: c.by,
            by_name,
            at_unix: c.at_unix,
        });
    }
    fired
}

/// The label and network id of the slot's network, read before the leave resets the key.
fn names_of(host: &HostSlots, id: &NetSlotId) -> (Option<String>, Option<String>) {
    let Some(slot) = host.slot(id) else {
        return (None, None);
    };
    let block = slot
        .config
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .network_block();
    let text = |key: &str| block.get(key).and_then(|v| v.as_str()).map(str::to_string);
    (text("label"), text("id_hex"))
}

fn leave_after_close(
    host: &HostSlots,
    id: &NetSlotId,
    ctl: &NodeCtl,
    label: Option<&str>,
) -> Fired {
    if host.is_stopping() {
        return Fired::Stopping;
    }
    if ctl.me_left() {
        return Fired::AlreadyGone;
    }
    match host.retire(id, true) {
        Ok(()) => Fired::Left,
        Err(HostError::UnknownNet) => Fired::AlreadyGone,
        Err(e) => {
            let shown = label.unwrap_or("the network");
            say(
                "host",
                format!(
                    "network {shown} was deleted by an admin; it stays until the node restarts without --key ({e:?})"
                ),
            );
            Fired::Stuck
        }
    }
}

#[cfg(test)]
#[path = "closed_watch_tests.rs"]
mod tests;
