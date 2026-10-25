//! Data of the tally: the input records, the exclusion and admission facts, and the resulting ledger.
//!
//! What it does: declares the plain types that go into and come out of `tally`, and `Ledger::is_member`.
//! What it doesn't do: no counting; the pass lives in `pass.rs`, the exclusion votes in `votes.rs`.

use std::collections::{BTreeMap, BTreeSet};

use crate::transport::net::PeerId;

use super::super::authority::PowerIn;
use super::super::record::Via;
use super::{ForgetIn, ForgetRound, Rules};

/// An admit record: `author` admits `subject`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdmitIn {
    pub author: PeerId,
    pub subject: PeerId,
    pub via: Via,
    pub made_unix: u64,
}

/// A live exclusion vote; `voter == subject` is a self-exclusion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoteIn {
    pub voter: PeerId,
    pub subject: PeerId,
    pub ts_micros: u64,
    /// The vote is counted against the threshold of its own moment; `false` — a record without the
    /// mark, counted against the current threshold.
    pub at_moment: bool,
}

/// Tally input: only live records (a withdrawn vote is simply absent).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TallyInput {
    pub me: PeerId,
    pub admits: Vec<AdmitIn>,
    pub votes: Vec<VoteIn>,
    pub forgets: Vec<ForgetIn>,
    pub rules: Rules,
    /// Records of power; only an administered network reads them.
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

/// A subject with votes that do not yet reach the threshold; `voters[0]` proposed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingVote {
    pub voters: Vec<PeerId>,
    pub need: u8,
}

/// A newcomer short of confirmations: `voters` are the counted authors in ascending order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Admission {
    pub voters: Vec<PeerId>,
    pub need: u8,
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
    /// All exclusions of the pass, also of nodes that later fell out of the closure.
    pub excluded: BTreeMap<PeerId, Exclusion>,
    pub pending: BTreeMap<PeerId, PendingVote>,
    /// Votes needed now.
    pub need: u8,
    /// Requests that do not reach the confirmation threshold yet.
    pub admissions: BTreeMap<PeerId, Admission>,
    /// Vote circles on `acl/forget/` records of non-excluded subjects.
    pub forgets: BTreeMap<PeerId, ForgetRound>,
    /// Latest admission time per subject among live admissions that count elsewhere.
    pub admit_unix: BTreeMap<PeerId, u64>,
    /// Latest `made_unix` per subject among all live forget records.
    pub forget_marks: BTreeMap<PeerId, u64>,
    /// Votes needed to forget a node now: the threshold of the forget pass, which counts the nodes that
    /// only a dead admission lets in, so it may differ from `need`.
    pub forget_need: u8,
    /// Administrators; empty in a peer network.
    pub admins: BTreeSet<PeerId>,
    /// Set when an administrator closed the network.
    pub closed: Option<Closed>,
}

impl Ledger {
    pub fn is_admin(&self, p: &PeerId) -> bool {
        self.admins.contains(p)
    }

    pub fn is_member(&self, p: &PeerId) -> bool {
        self.admitted.contains(p) && !self.excluded.contains_key(p) && !self.is_forgotten(p)
    }

    /// The way `p` was excluded, or `None` when `p` is not excluded.
    pub fn how_excluded(&self, p: &PeerId) -> Option<How> {
        self.excluded.get(p).map(|e| e.how)
    }
}
