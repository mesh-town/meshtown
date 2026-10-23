//! Forget votes (`acl/forget/` records): their types, the forget pass and the ledger accessors.
//!
//! What it does: one pass over admissions and forget votes in a total order of events. A vote joins the
//! circle of its subject; a circle that reaches the threshold forgets the subject, drops the pending votes of
//! the forgotten node and may complete further circles (a cascade at the same moment). An admission that
//! counts elsewhere, made by a counted author, clears the circle and returns the subject. Records made by a
//! node after it was forgotten do not count. The result depends on the set of records, not on their order.
//! What it doesn't do: no exclusion votes (the pass in `tally` owns them), no replica, no clock.

use std::collections::{BTreeMap, BTreeSet};

use crate::transport::net::PeerId;

use super::admission::counts_elsewhere;
use super::tally::{need_for, AdmitIn, Excluded, Ledger};

/// A live forget vote: `author` votes to forget `subject`, written at `made_unix`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ForgetIn {
    pub author: PeerId,
    pub subject: PeerId,
    pub made_unix: u64,
}

/// A vote circle on one subject; `voters[0]` proposed. `at_unix` is set once the subject is forgotten.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForgetRound {
    pub voters: Vec<PeerId>,
    pub at_unix: Option<u64>,
}

/// The result of the forget pass: circles of non-excluded subjects, and the admissions whose author
/// was not forgotten when they were made.
pub(super) struct ForgetOutcome {
    pub rounds: BTreeMap<PeerId, ForgetRound>,
    pub live_admits: Vec<AdmitIn>,
}

/// A pass event; the variant indexes the input slice.
#[derive(Clone, Copy)]
enum Event {
    Admit(usize),
    Vote(usize),
}

/// Sort key of an event: time, an admission before a vote of the same second, author, subject.
type EventKey = (u64, u8, PeerId, PeerId);

/// Events in pass order; a repeated (author, subject) vote keeps the first.
fn events(admits: &[AdmitIn], forgets: &[ForgetIn]) -> Vec<Event> {
    let admit_key =
        |(i, a): (usize, &AdmitIn)| ((a.made_unix, 0, a.author, a.subject), Event::Admit(i));
    let vote_key =
        |(i, f): (usize, &ForgetIn)| ((f.made_unix, 1, f.author, f.subject), Event::Vote(i));
    let mut keyed: Vec<(EventKey, Event)> = admits.iter().enumerate().map(admit_key).collect();
    let mut seen = BTreeSet::new();
    let mut votes: Vec<(EventKey, Event)> = forgets.iter().enumerate().map(vote_key).collect();
    votes.sort_by_key(|(k, _)| *k);
    votes.retain(|(k, _)| seen.insert((k.2, k.3)));
    keyed.extend(votes);
    keyed.sort_by_key(|(k, _)| *k);
    keyed.into_iter().map(|(_, e)| e).collect()
}

/// A circle under construction: votes as (made_unix, voter) in pass order.
#[derive(Default)]
struct Circle {
    votes: Vec<(u64, PeerId)>,
    at_unix: Option<u64>,
}

/// State of the forget pass.
struct Settle<'a> {
    closure0: &'a BTreeSet<PeerId>,
    excluded: &'a Excluded,
    circles: BTreeMap<PeerId, Circle>,
    live: Vec<AdmitIn>,
    need: u8,
}

impl Settle<'_> {
    fn forgotten(&self, p: &PeerId) -> bool {
        self.circles.get(p).is_some_and(|c| c.at_unix.is_some())
    }

    /// In the closure and not excluded.
    fn present(&self, p: &PeerId) -> bool {
        self.closure0.contains(p) && !self.excluded.contains_key(p)
    }

    /// An author whose records count right now.
    fn counts(&self, p: &PeerId) -> bool {
        self.present(p) && !self.forgotten(p)
    }

    fn refresh_need(&mut self) {
        let active = self.closure0.iter().filter(|p| self.counts(p)).count();
        self.need = need_for(active);
    }

    fn step(&mut self, event: Event, admits: &[AdmitIn], forgets: &[ForgetIn]) {
        match event {
            Event::Admit(i) => self.admit(&admits[i]),
            Event::Vote(i) => self.vote(&forgets[i]),
        }
    }

    /// An admission made by a forgotten author is dead; otherwise it may return its subject.
    fn admit(&mut self, a: &AdmitIn) {
        if self.forgotten(&a.author) {
            return;
        }
        self.live.push(*a);
        self.reopen(a);
    }

    fn reopen(&mut self, a: &AdmitIn) {
        if !counts_elsewhere(a.via) || a.author == a.subject || !self.counts(&a.author) {
            return;
        }
        self.circles.remove(&a.subject);
        self.refresh_need();
    }

    fn vote(&mut self, f: &ForgetIn) {
        if !self.counts(&f.author) || f.author == f.subject || !self.present(&f.subject) {
            return;
        }
        let circle = self.circles.entry(f.subject).or_default();
        if circle.votes.iter().any(|(_, v)| *v == f.author) {
            return;
        }
        circle.votes.push((f.made_unix, f.author));
        self.complete(f.subject, f.made_unix);
    }

    /// Forgets `subject` if its circle reaches the threshold, then cascades at the same moment.
    fn complete(&mut self, subject: PeerId, at: u64) {
        if self.forgotten(&subject) || self.circles[&subject].votes.len() < usize::from(self.need) {
            return;
        }
        self.forget(subject, at);
        while let Some(next) = self.candidate() {
            self.forget(next, at);
        }
    }

    fn forget(&mut self, x: PeerId, at: u64) {
        if let Some(c) = self.circles.get_mut(&x) {
            c.at_unix = Some(at);
        }
        for c in self.circles.values_mut().filter(|c| c.at_unix.is_none()) {
            c.votes.retain(|(_, v)| *v != x);
        }
        self.circles
            .retain(|_, c| c.at_unix.is_some() || !c.votes.is_empty());
        self.refresh_need();
    }

    /// The pending circle with enough votes whose first vote is the earliest.
    fn candidate(&self) -> Option<PeerId> {
        let need = usize::from(self.need);
        let enough = self
            .circles
            .iter()
            .filter(|(_, c)| c.at_unix.is_none() && c.votes.len() >= need);
        enough
            .min_by_key(|(s, c)| (c.votes[0], **s))
            .map(|(s, _)| *s)
    }

    fn outcome(self) -> ForgetOutcome {
        let round = |c: Circle| ForgetRound {
            voters: c.votes.into_iter().map(|(_, v)| v).collect(),
            at_unix: c.at_unix,
        };
        let rounds = self
            .circles
            .into_iter()
            .map(|(s, c)| (s, round(c)))
            .collect();
        ForgetOutcome {
            rounds,
            live_admits: self.live,
        }
    }
}

/// The forget pass over the closure `closure0` with the exclusions known so far.
pub(super) fn settle_forgets(
    closure0: &BTreeSet<PeerId>,
    excluded: &Excluded,
    admits: &[AdmitIn],
    forgets: &[ForgetIn],
) -> ForgetOutcome {
    let mut pass = Settle {
        closure0,
        excluded,
        circles: BTreeMap::new(),
        live: Vec::new(),
        need: 1,
    };
    pass.refresh_need();
    for event in events(admits, forgets) {
        pass.step(event, admits, forgets);
    }
    pass.outcome()
}

/// The latest time per subject over `(subject, made_unix)` pairs.
fn latest(rows: impl Iterator<Item = (PeerId, u64)>) -> BTreeMap<PeerId, u64> {
    let mut out = BTreeMap::new();
    for (p, t) in rows {
        let slot = out.entry(p).or_insert(t);
        *slot = (*slot).max(t);
    }
    out
}

/// Marks taken from the raw live records, regardless of the pass: the latest admission of each subject
/// that counts elsewhere, and the latest forget vote on each subject.
pub(super) fn marks(
    admits: &[AdmitIn],
    forgets: &[ForgetIn],
) -> (BTreeMap<PeerId, u64>, BTreeMap<PeerId, u64>) {
    let counted = admits.iter().filter(|a| counts_elsewhere(a.via));
    let admit_unix = latest(counted.map(|a| (a.subject, a.made_unix)));
    let forget_marks = latest(forgets.iter().map(|f| (f.subject, f.made_unix)));
    (admit_unix, forget_marks)
}

impl Ledger {
    pub fn is_forgotten(&self, p: &PeerId) -> bool {
        self.forgotten_at(p).is_some()
    }

    /// When the subject was forgotten, if it was.
    pub fn forgotten_at(&self, p: &PeerId) -> Option<u64> {
        self.forgets.get(p)?.at_unix
    }

    /// Latest `made_unix` among the live forget records about `p`.
    pub fn forget_mark(&self, p: &PeerId) -> Option<u64> {
        self.forget_marks.get(p).copied()
    }

    /// Latest `made_unix` among the live admissions of `p` that count elsewhere.
    pub fn last_admit_unix(&self, p: &PeerId) -> Option<u64> {
        self.admit_unix.get(p).copied()
    }
}
