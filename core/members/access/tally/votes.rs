//! Exclusion votes of the tally: the threshold, the pass order of the votes and the steps of the pass
//! that turn waiting votes into exclusions.
//!
//! What it does: orders the votes, counts them per subject, excludes a subject when the threshold is
//! reached and settles the cascade that an exclusion starts.
//! What it doesn't do: no admission and no forget settlement; those are refreshed by `Pass::refresh`.

use std::collections::{BTreeMap, BTreeSet};

use crate::transport::net::PeerId;

use super::super::admission::counts_elsewhere;
use super::pass::Pass;
use super::{AdmitIn, Exclusion, How, VoteIn};

pub(super) type VoteKey = (u64, PeerId, PeerId);
pub(in crate::members::access) type Excluded = BTreeMap<PeerId, Exclusion>;

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

/// Earliest `made_unix` per subject among admissions that count elsewhere.
pub(super) fn first_admits(admits: &[AdmitIn]) -> BTreeMap<PeerId, u64> {
    let mut first = BTreeMap::new();
    for a in admits.iter().filter(|a| counts_elsewhere(a.via)) {
        let at = first.entry(a.subject).or_insert(a.made_unix);
        *at = (*at).min(a.made_unix);
    }
    first
}

impl Pass {
    fn admitted_at(&self, p: &PeerId) -> u64 {
        self.first_admit.get(p).copied().unwrap_or(0)
    }

    /// Votes needed when only the nodes admitted by `moment_secs` count.
    fn need_at(&self, moment_secs: u64) -> u8 {
        let standing = self.standing();
        let active = self
            .admitted
            .iter()
            .filter(|p| standing.counts(p) && self.admitted_at(p) <= moment_secs);
        self.rules.need(active.count())
    }

    /// The threshold for the vote list of `subject`: of the list's own moment when every vote carries
    /// the mark, the current one otherwise.
    fn list_need(&self, subject: &PeerId, at_micros: u64) -> u8 {
        let Some(list) = self.pending.get(subject) else {
            return self.need;
        };
        if !list
            .iter()
            .all(|(_, v)| self.at_moment.contains(&(*v, *subject)))
        {
            return self.need;
        }
        let floor = list
            .iter()
            .map(|(_, v)| self.admitted_at(v))
            .max()
            .unwrap_or(0);
        let moment = (at_micros / 1_000_000)
            .max(self.admitted_at(subject))
            .max(floor);
        self.need_at(moment)
    }

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
        if v.at_moment {
            self.at_moment.insert((v.voter, v.subject));
        }
        let list = self.pending.entry(v.subject).or_default();
        list.push((vote_key(v), v.voter));
        let enough = list.len() >= usize::from(self.list_need(&v.subject, v.ts_micros));
        if enough {
            let list = &self.pending[&v.subject];
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
    fn candidate(&self, at: u64) -> Option<PeerId> {
        let enough = self
            .pending
            .iter()
            .filter(|(s, l)| l.len() >= usize::from(self.list_need(s, at)));
        enough.min_by_key(|(_, l)| l[0].0).map(|(s, _)| *s)
    }

    /// Settles the moment `at`: one exclusion per round until nobody reaches the threshold.
    /// `false` — the round cap was reached with a candidate left over.
    pub(super) fn settle(&mut self, at: u64, cap: usize) -> bool {
        for _ in 0..cap {
            self.refresh();
            let Some(subject) = self.candidate(at) else {
                return true;
            };
            let voters = self.pending[&subject].iter().map(|(_, v)| *v).collect();
            self.exclude(subject, How::Votes, at, voters);
        }
        self.refresh();
        self.candidate(at).is_none()
    }
}
