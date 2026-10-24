//! The pass of the tally: its state, the refresh of admission and forget circles, and the result.
//!
//! What it does: keeps the state of one forward pass, recomputes the admitted set and the forget circles
//! on demand, and turns the final state into a `Ledger`.
//! What it doesn't do: no vote counting; the exclusion steps of `Pass` live in `votes.rs`.

use std::collections::{BTreeMap, BTreeSet};

use crate::transport::net::PeerId;

use super::super::admission::admit_closure;
use super::super::authority::{Forgotten, Rules, Standing};
use super::super::forget::{marks, settle_forgets};
use super::votes::{sorted_votes, Excluded, VoteKey};
use super::{Admission, AdmitIn, ForgetIn, ForgetRound, Ledger, PendingVote, TallyInput};

/// State of the pass.
pub(super) struct Pass {
    pub(super) me: PeerId,
    pub(super) rules: Rules,
    pub(super) admits: Vec<AdmitIn>,
    pub(super) forgets: Vec<ForgetIn>,
    pub(super) admitted: BTreeSet<PeerId>,
    pub(super) excluded: Excluded,
    pub(super) pending: BTreeMap<PeerId, Vec<(VoteKey, PeerId)>>,
    pub(super) short: BTreeMap<PeerId, Admission>,
    pub(super) rounds: BTreeMap<PeerId, ForgetRound>,
    pub(super) forget_need: u8,
    pub(super) need: u8,
}

impl Pass {
    pub(super) fn new(input: &TallyInput) -> Pass {
        let mut pass = Pass {
            me: input.me,
            rules: input.rules,
            admits: input.admits.clone(),
            forgets: input.forgets.clone(),
            admitted: BTreeSet::new(),
            excluded: Excluded::new(),
            pending: BTreeMap::new(),
            short: BTreeMap::new(),
            rounds: BTreeMap::new(),
            forget_need: 1,
            need: 1,
        };
        pass.refresh();
        pass
    }

    /// In the closure and not excluded.
    pub(super) fn counts(&self, p: &PeerId) -> bool {
        self.standing().present(p)
    }

    fn standing(&self) -> Standing<'_> {
        Standing::new(&self.admitted, &self.excluded, &self.rounds)
    }

    /// Recomputes the forget circles and the admitted set, drops votes that no longer count and updates
    /// the threshold.
    pub(super) fn refresh(&mut self) {
        let closure0 = admit_closure(self.rules, self.me, &self.admits, &self.excluded).set;
        let settled = settle_forgets(
            self.rules,
            &closure0,
            &self.excluded,
            &self.admits,
            &self.forgets,
        );
        let next = admit_closure(self.rules, self.me, &settled.live_admits, &self.excluded);
        (self.admitted, self.short, self.rounds) = (next.set, next.short, settled.rounds);
        self.forget_need = settled.need;
        let mut pending = std::mem::take(&mut self.pending);
        pending.retain(|subject, _| self.counts(subject));
        for list in pending.values_mut() {
            list.retain(|((ts, _, _), voter)| self.voter_counts(voter, *ts));
        }
        pending.retain(|_, list| !list.is_empty());
        self.pending = pending;
        let standing = self.standing();
        let active = self.admitted.iter().filter(|p| standing.counts(p));
        self.need = self.rules.need(active.count());
    }

    pub(super) fn finish(mut self) -> Ledger {
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
            forget_need: self.forget_need,
        }
    }
}

impl Forgotten for BTreeMap<PeerId, ForgetRound> {
    fn forgotten(&self, p: &PeerId) -> bool {
        self.get(p).is_some_and(|r| r.at_unix.is_some())
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
