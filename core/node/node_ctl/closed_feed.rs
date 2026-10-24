//! The slot's permanent "network closed" channel. The node's access object owns the value; the feed forwards
//! it while the node is attached, so the channel keeps the current network's state across node cycles.

use std::sync::Arc;

use tokio::sync::watch;
use tokio::task::JoinHandle;

use super::state::NodeCtlShared;
use super::NodeCtl;
use crate::members::access::runtime::Access;
use crate::members::access::tally::Closed;
use crate::transport::net::PeerId;

/// Aborts a feed task and forgets it.
pub(super) fn abort_task(task: &mut Option<JoinHandle<()>>) {
    if let Some(task) = task.take() {
        task.abort();
    }
}

/// Forwards the access object's value to the slot's channel: the current one first, then every change.
pub(super) fn feed_closed(shared: Arc<NodeCtlShared>, access: Arc<Access>) -> JoinHandle<()> {
    let mut rx = access.closed();
    crate::node::output::spawn(async move {
        loop {
            let value = rx.borrow_and_update().clone();
            shared.closed.send_replace(value);
            if rx.changed().await.is_err() {
                return;
            }
        }
    })
}

impl NodeCtl {
    /// The closing of the slot's current network; `None` before the first `attach` and while no node is attached.
    pub(crate) fn closed(&self) -> watch::Receiver<Option<Closed>> {
        self.ctl_shared.closed.subscribe()
    }

    pub(super) fn start_closed_feed(&self, access: Arc<Access>) -> JoinHandle<()> {
        let shared = self.ctl_shared.clone();
        let _guard = self.rt.enter();
        feed_closed(shared, access)
    }

    /// The known name of a member of the current network; `None` without a node.
    pub(crate) fn peer_name(&self, id: &PeerId) -> Option<String> {
        let node = self.current_node()?;
        node.members.name_of(*id)
    }

    /// Whether the last verdicts count this node as having left. `false` without a node or verdicts.
    pub(crate) fn me_left(&self) -> bool {
        let Some(node) = self.current_node() else {
            return false;
        };
        let Some(v) = node.access.view.verdicts() else {
            return false;
        };
        let left = v.ledger.excluded.get(&v.me).map(|e| e.how);
        left == Some(crate::members::access::tally::How::Left)
    }

    fn current_node(&self) -> Option<Arc<crate::node::RunningNode>> {
        let st = self
            .ctl_shared
            .ctl_state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        st.node.clone()
    }

    #[cfg(test)]
    pub(crate) fn set_closed_for_test(&self, v: Option<Closed>) {
        self.ctl_shared.closed.send_replace(v);
    }
}

#[cfg(test)]
#[path = "closed_feed_tests.rs"]
mod tests;
