//! The controller's link to the process memory guard: the slot id it writes under and the shared
//! sources it writes to.

use super::state::NodeCtl;
use crate::config::config_file::NetSlotId;
use crate::members::membership::Members;
use crate::node::guard_sources::GuardSources;
use crate::site_checks::checks::Checks;

impl NodeCtl {
    /// The process host gives the controller the guard's sources and the slot id to write under;
    /// the first call wins.
    pub(crate) fn set_guard_sources(&self, id: NetSlotId, sources: GuardSources) {
        let _ = self.guard_sources.set((id, sources));
    }

    /// The guard's sources given by the host (tests only).
    #[cfg(test)]
    pub(crate) fn guard_sources(&self) -> Option<GuardSources> {
        self.guard_sources.get().map(|(_, s)| s.clone())
    }

    /// The node is up: the guard's summary reads its handles.
    pub(super) fn publish_guard_sources(&self, members: Members, checks: Checks) {
        let link = self.guard_sources.get();
        link.iter()
            .for_each(|(id, s)| s.set(id.clone(), members.clone(), checks.clone()));
    }

    /// The node has stopped: the guard's summary no longer reads it.
    pub(crate) fn release_guard_sources(&self) {
        self.guard_sources
            .get()
            .iter()
            .for_each(|(id, s)| s.clear(id));
    }
}
