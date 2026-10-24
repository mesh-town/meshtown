//! Exclusion votes of the tally: the threshold, the pass order of the votes and the steps of the pass
//! that turn waiting votes into exclusions.
//!
//! What it does: orders the votes, counts them per subject, excludes a subject when the threshold is
//! reached and settles the cascade that an exclusion starts.
//! What it doesn't do: no admission and no forget settlement; those are refreshed by `Pass::refresh`.

use std::collections::{BTreeMap, BTreeSet};

use crate::transport::net::PeerId;

use super::pass::Pass;
use super::{Exclusion, How, VoteIn};

pub(super) type VoteKey = (u64, PeerId, PeerId);
pub(in crate::members::access) type Excluded = BTreeMap<PeerId, Exclusion>;

pub(in crate::members::access) fn need_for(active: usize) -> u8 {
    if active <= 2 {
        return 1;
    }
    2
}

pub(super) fn vote_key(v: &VoteIn) -> VoteKey {
    (v.ts_micros, v.voter, v.subject)
}

/// Votes in pass order; a repeated (voter, subject) pair keeps the first.
pub(super) fn sorted_votes(votes: &[VoteIn]) -> Vec<VoteIn> {
    let mut sorted = votes.to_vec();
    sorted.sort_by_key(vote_key);
    let mut seen = BTreeSet::new();
    sorted.retain(|v| seen.insert((v.voter, v.subject)));
    sorted
}

impl Pass {
    /// A voter counts if present and, when forgotten, only for votes made by the moment of forgetting.
    pub(super) fn voter_counts(&self, voter: &PeerId, ts_micros: u64) -> bool {
        let at = self.rounds.get(voter).and_then(|r| r.at_unix);
        self.counts(voter) && at.is_none_or(|at| ts_micros / 1_000_000 <= at)
    }

    pub(super) fn vote(&mut self, v: &VoteIn) {
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
    pub(super) fn settle(&mut self, at: u64, cap: usize) -> bool {
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
}
