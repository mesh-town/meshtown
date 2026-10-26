//! Data of the tally of the membership list: the input records and the resulting ledger.
//!
//! What it does: declares the plain types that go into and come out of `tally`, and the queries on
//! `Ledger`. The counting itself is `managed::tally`, re-exported here.
//! What it doesn't do: no counting, no replica, no clock, no I/O.

use std::collections::{BTreeMap, BTreeSet};

use crate::transport::net::PeerId;

use super::authority::PowerIn;

pub use super::managed::tally;

/// A live exclusion vote; `voter == subject` is a self-exclusion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoteIn {
    pub voter: PeerId,
    pub subject: PeerId,
    pub ts_micros: u64,
}

/// Tally input: only live records (a withdrawn vote is simply absent).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TallyInput {
    pub me: PeerId,
    /// The creator of the network and its first administrator.
    pub root: PeerId,
    pub votes: Vec<VoteIn>,
    /// Records of power: admissions, grants, revocations and the closing of the network.
    pub powers: Vec<PowerIn>,
}

/// How a node was excluded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum How {
    Votes,
    Left,
}

/// An exclusion: when and by whom, voters in time order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Exclusion {
    pub how: How,
    pub at_micros: u64,
    pub voters: Vec<PeerId>,
}

/// A newcomer an administrator asked for who is not a member yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Admission {
    pub label: Option<String>,
}

/// The closing of a network by an administrator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Closed {
    pub by: PeerId,
    pub at_unix: u64,
}

/// The result of a tally.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ledger {
    /// Admitted set at the end of the pass; includes excluded nodes.
    pub admitted: BTreeSet<PeerId>,
    /// All exclusions of the pass.
    pub excluded: BTreeMap<PeerId, Exclusion>,
    /// Requests that do not make their subject a member yet.
    pub admissions: BTreeMap<PeerId, Admission>,
    pub admins: BTreeSet<PeerId>,
    /// Set when an administrator closed the network.
    pub closed: Option<Closed>,
}

impl Ledger {
    /// A ledger with nobody in it.
    pub fn empty() -> Ledger {
        Ledger {
            admitted: BTreeSet::new(),
            excluded: BTreeMap::new(),
            admissions: BTreeMap::new(),
            admins: BTreeSet::new(),
            closed: None,
        }
    }

    pub fn is_admin(&self, p: &PeerId) -> bool {
        self.admins.contains(p)
    }

    pub fn is_member(&self, p: &PeerId) -> bool {
        self.admitted.contains(p) && !self.excluded.contains_key(p)
    }

    /// The way `p` was excluded, or `None` when `p` is not excluded.
    pub fn how_excluded(&self, p: &PeerId) -> Option<How> {
        self.excluded.get(p).map(|e| e.how)
    }
}
