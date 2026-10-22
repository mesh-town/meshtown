//! What the memory guard's summary reads from the live nodes: one `Members` and `Checks` pair per
//! network slot. The process keeps one guard, nodes come and go, so each node's controller puts its
//! handles here under its slot id and takes them away.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use crate::config::config_file::NetSlotId;
use crate::members::membership::Members;
use crate::site_checks::checks::Checks;

/// The handles of one live node.
pub(crate) type NodeHandles = (Members, Checks);

/// Shared map `slot id → handles of the slot's live node`.
#[derive(Clone, Default)]
pub(crate) struct GuardSources(Arc<Mutex<BTreeMap<NetSlotId, NodeHandles>>>);

impl GuardSources {
    /// The slot's node is up: the summary reads these handles.
    pub(crate) fn set(&self, id: NetSlotId, members: Members, checks: Checks) {
        self.lock().insert(id, (members, checks));
    }

    /// The slot's node is gone: the summary has nothing to read for it.
    pub(crate) fn clear(&self, id: &NetSlotId) {
        self.lock().remove(id);
    }

    /// The handles of every live node, in slot id order; empty — no node.
    pub(crate) fn snapshot(&self) -> Vec<NodeHandles> {
        self.lock().values().cloned().collect()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<NetSlotId, NodeHandles>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}
