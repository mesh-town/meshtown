//! The counter the admission gate writes to and the page of the owner reads.

use std::collections::HashMap;
use std::time::Instant;

use super::buckets::{index_of, Ring, SLOTS};
use super::counts::{MemberCounts, Outcome, ProcCounts};
use super::verdict::{decide, Reason};
use crate::transport::net::PeerId;

/// Members listed for the owner at most.
const LISTED_MEMBERS: usize = 20;

/// Decisions about one member over the kept ten minutes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MemberLoad {
    pub(crate) peer: PeerId,
    pub(crate) admitted: u64,
    pub(crate) refused: u64,
}

/// Numbers for the owner's page: the process totals over ten minutes and members with refusals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LoadSnapshot {
    pub(crate) overloaded: Option<Reason>,
    pub(crate) admitted: u64,
    pub(crate) refused: u64,
    pub(crate) members: Vec<MemberLoad>,
}

/// Decisions of the process and of each member in 15-second buckets, plus the one bit of state.
pub(crate) struct Meter {
    origin: Instant,
    procs: Ring<ProcCounts>,
    members: HashMap<PeerId, Ring<MemberCounts>>,
    overloaded: bool,
}

impl Meter {
    pub(crate) fn new(origin: Instant) -> Self {
        Self {
            origin,
            procs: Ring::new(),
            members: HashMap::new(),
            overloaded: false,
        }
    }

    pub(crate) fn record(&mut self, peer: PeerId, outcome: Outcome, now: Instant) {
        let index = index_of(self.origin, now);
        self.procs.at(index).note(outcome);
        self.members
            .entry(peer)
            .or_insert_with(Ring::new)
            .at(index)
            .note(outcome);
        self.forget_idle(index);
    }

    fn forget_idle(&mut self, index: u64) {
        self.members.retain(|_, ring| !ring.is_stale(index));
    }

    /// Recomputes the state at `now`, keeps the bit, returns the reason while overloaded.
    pub(crate) fn overloaded(&mut self, now: Instant) -> Option<Reason> {
        let index = index_of(self.origin, now);
        let verdict = decide(self.overloaded, &self.procs, self.members.values(), index);
        self.overloaded = verdict.overloaded;
        verdict.reason
    }

    /// Members with events in the last ten minutes, most refused first.
    pub(crate) fn members(&mut self, now: Instant) -> Vec<MemberLoad> {
        let index = index_of(self.origin, now);
        self.forget_idle(index);
        let mut list: Vec<_> = self
            .members
            .iter()
            .map(|(p, r)| load_of(*p, r, index))
            .collect();
        list.sort_by(|a, b| b.refused.cmp(&a.refused).then(a.peer.cmp(&b.peer)));
        list
    }

    pub(crate) fn snapshot(&mut self, now: Instant) -> LoadSnapshot {
        let overloaded = self.overloaded(now);
        let index = index_of(self.origin, now);
        let total = self.procs.total(index, SLOTS);
        let mut members = self.members(now);
        members.retain(|m| m.refused > 0);
        members.truncate(LISTED_MEMBERS);
        LoadSnapshot {
            overloaded,
            admitted: total.admitted,
            refused: total.refused(),
            members,
        }
    }

    #[cfg(test)]
    pub(super) fn member_count(&self) -> usize {
        self.members.len()
    }
}

fn load_of(peer: PeerId, ring: &Ring<MemberCounts>, index: u64) -> MemberLoad {
    let sum = ring.total(index, SLOTS);
    MemberLoad {
        peer,
        admitted: sum.admitted,
        refused: sum.refused,
    }
}

#[cfg(test)]
#[path = "meter_tests.rs"]
mod tests;
