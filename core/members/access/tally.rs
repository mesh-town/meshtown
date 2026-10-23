//! Vote tally of the membership list: a pure deterministic function from admit records and
//! exclusion votes to the ledger (who is a member, who is excluded, what is pending).
//!
//! What it does: one forward pass over the votes in time order. Who is admitted comes from
//! `admission::admit_closure`: one confirmation per newcomer, a fixed point independent of the order.
//! Forget votes are settled by `forget::settle_forgets` on every recount; admissions a node made after it was
//! forgotten leave the closure, and a forgotten node's exclusion votes count only up to its forgetting.
//! An exclusion narrows the reach and lowers the exclusion thresholds, so a pending removal may complete during the
//! pass, but the excluded only grow, so there is no oscillation and a cascade is the end of a pass.
//! What it doesn't do: no replica, no clock, no I/O; blocks, the boot marker and ignored entries are not its business.

use std::collections::{BTreeMap, BTreeSet};

use crate::transport::net::PeerId;

use super::admission::admit_closure;
use super::forget::{marks, settle_forgets};
use super::record::Via;

pub use super::forget::{ForgetIn, ForgetRound};

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
}

/// Tally input: only live records (a withdrawn vote is simply absent).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TallyInput {
    pub me: PeerId,
    pub admits: Vec<AdmitIn>,
    pub votes: Vec<VoteIn>,
    pub forgets: Vec<ForgetIn>,
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
}

impl Ledger {
    pub fn is_member(&self, p: &PeerId) -> bool {
        self.admitted.contains(p) && !self.excluded.contains_key(p) && !self.is_forgotten(p)
    }
}

type VoteKey = (u64, PeerId, PeerId);
pub(super) type Excluded = BTreeMap<PeerId, Exclusion>;

pub(super) fn need_for(active: usize) -> u8 {
    if active <= 2 {
        return 1;
    }
    2
}

fn vote_key(v: &VoteIn) -> VoteKey {
    (v.ts_micros, v.voter, v.subject)
}

/// Votes in pass order; a repeated (voter, subject) pair keeps the first.
fn sorted_votes(votes: &[VoteIn]) -> Vec<VoteIn> {
    let mut sorted = votes.to_vec();
    sorted.sort_by_key(vote_key);
    let mut seen = BTreeSet::new();
    sorted.retain(|v| seen.insert((v.voter, v.subject)));
    sorted
}

/// State of the pass.
struct Pass {
    me: PeerId,
    admits: Vec<AdmitIn>,
    forgets: Vec<ForgetIn>,
    admitted: BTreeSet<PeerId>,
    excluded: Excluded,
    pending: BTreeMap<PeerId, Vec<(VoteKey, PeerId)>>,
    short: BTreeMap<PeerId, Admission>,
    rounds: BTreeMap<PeerId, ForgetRound>,
    need: u8,
}

impl Pass {
    fn new(input: &TallyInput) -> Pass {
        let mut pass = Pass {
            me: input.me,
            admits: input.admits.clone(),
            forgets: input.forgets.clone(),
            admitted: BTreeSet::new(),
            excluded: Excluded::new(),
            pending: BTreeMap::new(),
            short: BTreeMap::new(),
            rounds: BTreeMap::new(),
            need: 1,
        };
        pass.refresh();
        pass
    }

    /// In the closure and not excluded.
    fn counts(&self, p: &PeerId) -> bool {
        self.admitted.contains(p) && !self.excluded.contains_key(p)
    }

    fn forgotten(&self, p: &PeerId) -> bool {
        self.rounds.get(p).is_some_and(|r| r.at_unix.is_some())
    }

    /// A voter counts if present and, when forgotten, only for votes made by the moment of forgetting.
    fn voter_counts(&self, voter: &PeerId, ts_micros: u64) -> bool {
        let at = self.rounds.get(voter).and_then(|r| r.at_unix);
        self.counts(voter) && at.is_none_or(|at| ts_micros / 1_000_000 <= at)
    }

    fn vote(&mut self, v: &VoteIn) {
        if !self.voter_counts(&v.voter, v.ts_micros) || !self.counts(&v.subject) {
            return;
        }
        let cap = self.admitted.len() + 1;
        if v.voter == v.subject {
            self.exclude(v.subject, How::Left, v.ts_micros, vec![v.subject]);
            self.settle(v.ts_micros, cap);
            return;
        }
        let list = self.pending.entry(v.subject).or_default();
        list.push((vote_key(v), v.voter));
        if list.len() >= usize::from(self.need) {
            let voters = list.iter().map(|(_, voter)| *voter).collect();
            self.exclude(v.subject, How::Votes, v.ts_micros, voters);
            self.settle(v.ts_micros, cap);
        }
    }

    fn exclude(&mut self, subject: PeerId, how: How, at: u64, voters: Vec<PeerId>) {
        self.pending.remove(&subject);
        self.excluded.entry(subject).or_insert(Exclusion {
            how,
            at_micros: at,
            voters,
        });
    }

    /// Recomputes the forget circles and the admitted set, drops votes that no longer count and updates
    /// the threshold.
    fn refresh(&mut self) {
        let closure0 = admit_closure(self.me, &self.admits, &self.excluded).set;
        let settled = settle_forgets(&closure0, &self.excluded, &self.admits, &self.forgets);
        let next = admit_closure(self.me, &settled.live_admits, &self.excluded);
        (self.admitted, self.short, self.rounds) = (next.set, next.short, settled.rounds);
        let mut pending = std::mem::take(&mut self.pending);
        pending.retain(|subject, _| self.counts(subject));
        for list in pending.values_mut() {
            list.retain(|((ts, _, _), voter)| self.voter_counts(voter, *ts));
        }
        pending.retain(|_, list| !list.is_empty());
        self.pending = pending;
        let active = self
            .admitted
            .iter()
            .filter(|p| self.counts(p) && !self.forgotten(p));
        self.need = need_for(active.count());
    }

    /// The subject with enough votes whose first vote is the earliest.
    fn candidate(&self) -> Option<PeerId> {
        let enough = self
            .pending
            .iter()
            .filter(|(_, l)| l.len() >= usize::from(self.need));
        enough.min_by_key(|(_, l)| l[0].0).map(|(s, _)| *s)
    }

    /// Settles the moment `at`: one exclusion per round until nobody reaches the threshold.
    /// `false` — the round cap was reached with a candidate left over.
    fn settle(&mut self, at: u64, cap: usize) -> bool {
        for _ in 0..cap {
            self.refresh();
            let Some(subject) = self.candidate() else {
                return true;
            };
            let voters = self.pending[&subject].iter().map(|(_, v)| *v).collect();
            self.exclude(subject, How::Votes, at, voters);
        }
        self.refresh();
        self.candidate().is_none()
    }

    fn finish(mut self) -> Ledger {
        self.refresh();
        let need = self.need;
        let to_pending = |l: Vec<(VoteKey, PeerId)>| PendingVote {
            voters: l.into_iter().map(|(_, v)| v).collect(),
            need,
        };
        let pending = self
            .pending
            .into_iter()
            .map(|(s, l)| (s, to_pending(l)))
            .collect();
        Ledger {
            admitted: self.admitted,
            excluded: self.excluded,
            pending,
            need,
            admissions: self.short,
            forgets: self.rounds,
            admit_unix: BTreeMap::new(),
            forget_marks: BTreeMap::new(),
        }
    }
}

/// Computes the ledger of the node `input.me`.
pub fn tally(input: &TallyInput) -> Ledger {
    let mut pass = Pass::new(input);
    for v in sorted_votes(&input.votes) {
        pass.vote(&v);
    }
    let mut ledger = pass.finish();
    (ledger.admit_unix, ledger.forget_marks) = marks(&input.admits, &input.forgets);
    ledger
}

#[cfg(test)]
#[path = "tally_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tally_cascade_tests.rs"]
mod cascade_tests;

#[cfg(test)]
#[path = "tally_forget_tests.rs"]
mod forget_tests;

#[cfg(test)]
#[path = "tally_forget_vote_tests.rs"]
mod forget_vote_tests;
