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
use super::votes::{first_admits, sorted_votes, Excluded, VoteKey};
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
    /// Earliest counted admission time per node; absent — admitted since always.
    pub(super) first_admit: BTreeMap<PeerId, u64>,
    /// (voter, subject) of the votes that carry the mark.
    pub(super) at_moment: BTreeSet<(PeerId, PeerId)>,
}

impl Pass {
    pub(super) fn new(input: &TallyInput) -> Pass {
        let mut pass = Pass {
            admits: input.admits.clone(),
            forgets: input.forgets.clone(),
            first_admit: first_admits(&input.admits),
            ..Pass::empty(input.me, input.rules)
        };
        pass.refresh();
        pass
    }

    fn empty(me: PeerId, rules: Rules) -> Pass {
        Pass {
            me,
            rules,
            admits: Vec::new(),
            forgets: Vec::new(),
            admitted: BTreeSet::new(),
            excluded: Excluded::new(),
            pending: BTreeMap::new(),
            short: BTreeMap::new(),
            rounds: BTreeMap::new(),
            forget_need: 1,
            need: 1,
            first_admit: BTreeMap::new(),
            at_moment: BTreeSet::new(),
        }
    }

    /// In the closure and not excluded.
    pub(super) fn counts(&self, p: &PeerId) -> bool {
        self.standing().present(p)
    }

    pub(super) fn standing(&self) -> Standing<'_> {
        Standing::new(&self.admitted, &self.excluded, &self.rounds)
    }

    /// Recomputes the forget circles and the admitted set, drops votes that no longer count and updates
    /// the threshold.
    pub(super) fn refresh(&mut self) {
        self.settle_circles();
        self.drop_stale_votes();
        self.update_need();
    }

    /// The admitted set and the forget circles, each computed against the other.
    fn settle_circles(&mut self) {
        let (rules, me) = (self.rules, self.me);
        let closure0 = admit_closure(rules, me, &self.admits, &self.excluded).set;
        let (admits, forgets) = (&self.admits, &self.forgets);
        let settled = settle_forgets(rules, &closure0, &self.excluded, admits, forgets);
        let next = admit_closure(rules, me, &settled.live_admits, &self.excluded);
        (self.admitted, self.short, self.rounds) = (next.set, next.short, settled.rounds);
        self.forget_need = settled.need;
    }

    /// Pending votes of subjects that left the count, and votes of voters that no longer count.
    fn drop_stale_votes(&mut self) {
        let mut pending = std::mem::take(&mut self.pending);
        pending.retain(|subject, _| self.counts(subject));
        for list in pending.values_mut() {
            list.retain(|((ts, _, _), voter)| self.voter_counts(voter, *ts));
        }
        pending.retain(|_, list| !list.is_empty());
        self.pending = pending;
    }

    fn update_need(&mut self) {
        let standing = self.standing();
        let active = self.admitted.iter().filter(|p| standing.counts(p));
        self.need = self.rules.need(active.count());
    }

    pub(super) fn finish(mut self) -> Ledger {
        self.refresh();
        let need = self.need;
        let pending = self.pending_votes(need);
        Ledger {
            admitted: self.admitted,
            excluded: self.excluded,
            pending,
            need,
            admissions: self.short,
            forgets: self.rounds,
            forget_need: self.forget_need,
            ..Ledger::bare()
        }
    }

    fn pending_votes(&mut self, need: u8) -> BTreeMap<PeerId, PendingVote> {
        let pending = std::mem::take(&mut self.pending);
        let to_vote = |l: Vec<(VoteKey, PeerId)>| PendingVote {
            voters: l.into_iter().map(|(_, v)| v).collect(),
            need,
        };
        pending.into_iter().map(|(s, l)| (s, to_vote(l))).collect()
    }
}

impl Ledger {
    /// A ledger with nothing in it; the pass fills the counted parts, the caller the marks.
    fn bare() -> Ledger {
        Ledger {
            admitted: BTreeSet::new(),
            excluded: BTreeMap::new(),
            pending: BTreeMap::new(),
            need: 1,
            admissions: BTreeMap::new(),
            forgets: BTreeMap::new(),
            admit_unix: BTreeMap::new(),
            forget_marks: BTreeMap::new(),
            forget_need: 1,
            admins: BTreeSet::new(),
            closed: None,
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
    if let Some(root) = input.rules.root() {
        return super::super::managed::tally(root, input);
    }
    let mut pass = Pass::new(input);
    for v in sorted_votes(&input.votes) {
        pass.vote(&v);
    }
    let mut ledger = pass.finish();
    (ledger.admit_unix, ledger.forget_marks) = marks(&input.admits, &input.forgets);
    ledger
}
