//! One pass over the timeline of an administered network: who is a member, who is an administrator,
//! who is excluded, whether the network is closed.
//!
//! What it does: replays the events in order. A record of power counts when its author is an
//! administrator at that moment and the author's earlier records were not voided by an exclusion.
//! Exclusions pinned by the caller stand whoever wrote them.
//! What it doesn't do: no fixed point over the voided records; that is `closure.rs`.

use std::collections::{BTreeMap, BTreeSet};

use crate::transport::net::PeerId;

use super::super::record::Via;
use super::super::tally::{Closed, Exclusion, How};
use super::events::{Act, Event, EventKey};

/// The state after a pass.
pub(super) struct Replay {
    root: PeerId,
    /// Per node: records at or before this time are void.
    cutoff: BTreeMap<PeerId, u64>,
    committed: BTreeSet<EventKey>,
    pub(super) members: BTreeSet<PeerId>,
    pub(super) admins: BTreeSet<PeerId>,
    /// Everybody who was a member at some point.
    pub(super) admitted: BTreeSet<PeerId>,
    pub(super) excluded: BTreeMap<PeerId, Exclusion>,
    left: BTreeSet<PeerId>,
    pub(super) closed: Option<Closed>,
    /// Exclusions that took effect in this pass.
    pub(super) counted: BTreeSet<EventKey>,
}

impl Replay {
    /// Replays `events` with the pinned exclusions `committed`.
    pub(super) fn run(events: &[Event], root: PeerId, committed: &BTreeSet<EventKey>) -> Replay {
        let mut cutoff: BTreeMap<PeerId, u64> = BTreeMap::new();
        for (ts, act, _, subject) in committed {
            if *act == Act::Exclude {
                let at = cutoff.entry(*subject).or_insert(0);
                *at = (*at).max(*ts);
            }
        }
        let start = BTreeSet::from([root]);
        let mut replay = Replay {
            root,
            cutoff,
            committed: committed.clone(),
            members: start.clone(),
            admins: start.clone(),
            admitted: start,
            excluded: BTreeMap::new(),
            left: BTreeSet::new(),
            closed: None,
            counted: BTreeSet::new(),
        };
        for e in events {
            replay.apply(e);
            if replay.closed.is_some() {
                break;
            }
        }
        replay
    }

    /// The author is an administrator now and the record is not voided.
    fn fit(&self, e: &Event) -> bool {
        let cut = self.cutoff.get(&e.author).copied().unwrap_or(0);
        self.admins.contains(&e.author) && e.ts > cut
    }

    fn apply(&mut self, e: &Event) {
        match e.act {
            Act::Admit => self.admit(e),
            Act::Grant => self.grant(e),
            Act::Revoke => self.revoke(e),
            Act::Exclude => self.exclude(e),
            Act::Left => self.leave(e),
            Act::Close => self.close(e),
        }
    }

    fn admit(&mut self, e: &Event) {
        let out = e.subject == e.author || e.subject == self.root;
        if !self.fit(e) || e.via == Via::Witness || out {
            return;
        }
        if self.left.contains(&e.subject) || self.members.contains(&e.subject) {
            return;
        }
        self.members.insert(e.subject);
        self.admitted.insert(e.subject);
        self.excluded.remove(&e.subject);
    }

    fn grant(&mut self, e: &Event) {
        if self.fit(e) && self.members.contains(&e.subject) {
            self.admins.insert(e.subject);
        }
    }

    fn revoke(&mut self, e: &Event) {
        let last = self.admins.len() < 2;
        if self.fit(e) && self.admins.contains(&e.subject) && e.subject != self.root && !last {
            self.admins.remove(&e.subject);
        }
    }

    fn exclude(&mut self, e: &Event) {
        let pinned = self.committed.contains(&e.key());
        if !(pinned || self.fit(e)) || e.subject == self.root || !self.members.contains(&e.subject)
        {
            return;
        }
        self.members.remove(&e.subject);
        self.admins.remove(&e.subject);
        let how = Exclusion {
            how: How::Votes,
            at_micros: e.ts,
            voters: vec![e.author],
        };
        self.excluded.insert(e.subject, how);
        self.counted.insert(e.key());
    }

    fn leave(&mut self, e: &Event) {
        self.left.insert(e.subject);
        self.members.remove(&e.subject);
        self.admins.remove(&e.subject);
        if self.admitted.contains(&e.subject) {
            let gone = Exclusion {
                how: How::Left,
                at_micros: e.ts,
                voters: vec![e.subject],
            };
            self.excluded.entry(e.subject).or_insert(gone);
        }
    }

    fn close(&mut self, e: &Event) {
        if self.fit(e) {
            self.closed = Some(Closed {
                by: e.author,
                at_unix: e.ts / 1_000_000,
            });
        }
    }
}

#[cfg(test)]
#[path = "roles_tests.rs"]
mod tests;
