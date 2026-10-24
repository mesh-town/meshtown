//! Who may count: the counting rules of a network and the standing of its nodes.
//!
//! What it does: `Rules` holds what differs between kinds of network (the seeds of the admitted
//! closure, the exclusion threshold, whose admission records count); `Standing` answers whether a node
//! is in the closure, excluded or forgotten, so the passes of the tally ask one place.
//! What it doesn't do: no counting of votes, no records, no replica.

use std::collections::BTreeSet;

use crate::transport::net::PeerId;

use crate::members::network_key::NetKind;

use super::admission::counts_elsewhere;
use super::record::{AclKind, Via};
use super::tally::Excluded;

/// The counting rules of a network.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Rules {
    /// Every admitted node may vote.
    #[default]
    Peer,
    /// Administrators decide; `root` is the creator and always an administrator.
    Managed { root: PeerId },
}

/// What a record of power says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Power {
    /// An admission by the given origin.
    Admit(Via),
    Grant,
    Revoke,
    Close,
}

/// A live record of power: `author` writes `power` about `subject` at `ts_micros`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PowerIn {
    pub author: PeerId,
    pub power: Power,
    pub subject: PeerId,
    pub ts_micros: u64,
}

/// The power a record kind carries; `None` for kinds that are not records of power.
pub(super) fn power_of(kind: AclKind, via: Via) -> Option<Power> {
    match kind {
        AclKind::Admit => Some(Power::Admit(via)),
        AclKind::Grant => Some(Power::Grant),
        AclKind::Revoke => Some(Power::Revoke),
        AclKind::Close => Some(Power::Close),
        _ => None,
    }
}

impl From<&NetKind> for Rules {
    fn from(kind: &NetKind) -> Rules {
        match kind.root() {
            Some(root) => Rules::Managed { root },
            None => Rules::Peer,
        }
    }
}

impl Rules {
    /// The creator of an administered network.
    pub fn root(&self) -> Option<PeerId> {
        match self {
            Rules::Peer => None,
            Rules::Managed { root } => Some(*root),
        }
    }

    /// The nodes the admitted closure starts from.
    pub fn seeds(&self, me: PeerId) -> BTreeSet<PeerId> {
        match self {
            Rules::Peer => BTreeSet::from([me]),
            Rules::Managed { root } => BTreeSet::from([*root]),
        }
    }

    /// Votes needed to remove a node when `active` nodes count.
    pub fn need(&self, active: usize) -> u8 {
        match self {
            Rules::Peer => peer_need(active),
            Rules::Managed { .. } => 1,
        }
    }

    /// Whether an admission record by `author` counts as a vote at the node `me`.
    pub fn admit_counts(&self, via: Via, author: &PeerId, me: &PeerId) -> bool {
        match self {
            Rules::Peer => counts_elsewhere(via) || author == me,
            Rules::Managed { .. } => via != Via::Witness,
        }
    }

    /// Whether the network is administered.
    pub fn is_managed(&self) -> bool {
        self.root().is_some()
    }
}

fn peer_need(active: usize) -> u8 {
    if active <= 2 {
        return 1;
    }
    2
}

/// Tells which nodes were forgotten.
pub(super) trait Forgotten {
    fn forgotten(&self, p: &PeerId) -> bool;
}

/// No node is forgotten.
impl Forgotten for () {
    fn forgotten(&self, _: &PeerId) -> bool {
        false
    }
}

/// A snapshot of who is in the closure, who is excluded and who is forgotten.
#[derive(Clone, Copy)]
pub(super) struct Standing<'a> {
    closure: &'a BTreeSet<PeerId>,
    excluded: &'a Excluded,
    roll: &'a dyn Forgotten,
}

impl<'a> Standing<'a> {
    pub(super) fn new(
        closure: &'a BTreeSet<PeerId>,
        excluded: &'a Excluded,
        roll: &'a dyn Forgotten,
    ) -> Standing<'a> {
        Standing {
            closure,
            excluded,
            roll,
        }
    }

    /// In the closure and not excluded.
    pub(super) fn present(&self, p: &PeerId) -> bool {
        self.closure.contains(p) && !self.excluded.contains_key(p)
    }

    /// Present and not forgotten: the records of this node count right now.
    pub(super) fn counts(&self, p: &PeerId) -> bool {
        self.present(p) && !self.roll.forgotten(p)
    }

    pub(super) fn excluded(&self, p: &PeerId) -> bool {
        self.excluded.contains_key(p)
    }

    /// An author whose records are read: `me`, or anybody not excluded.
    pub(super) fn open(&self, me: &PeerId, author: &PeerId) -> bool {
        author == me || !self.excluded(author)
    }

    /// In the closure and open.
    pub(super) fn backs(&self, me: &PeerId, author: &PeerId) -> bool {
        self.closure.contains(author) && self.open(me, author)
    }
}

#[cfg(test)]
#[path = "authority_tests.rs"]
mod tests;
