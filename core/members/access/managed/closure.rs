//! The tally of an administered network: the pass over the timeline repeated until the set of pinned
//! exclusions stops growing, and the ledger built from the result.
//!
//! What it does: an exclusion of an administrator voids that administrator's earlier records, which can
//! void other exclusions. Every exclusion that took effect is pinned for the next pass and stays even if
//! its author is voided later, so the set only grows and the loop ends.
//! What it doesn't do: no I/O; the input is already parsed.

use std::collections::{BTreeMap, BTreeSet};

use crate::transport::net::PeerId;

use super::super::tally::{Admission, Ledger, TallyInput};
use super::events::{events, Act, Event, EventKey};
use super::roles::Replay;

/// Computes the ledger of the node `input.me` in a network created by `root`.
pub fn tally(root: PeerId, input: &TallyInput) -> Ledger {
    let all = events(input);
    let limit = all.iter().filter(|e| e.act == Act::Exclude).count() + 1;
    let mut pinned: BTreeSet<EventKey> = BTreeSet::new();
    let mut rounds = 0;
    let replay = loop {
        let replay = Replay::run(&all, root, &pinned);
        if replay.counted.is_subset(&pinned) {
            break replay;
        }
        pinned.extend(replay.counted.iter().copied());
        rounds += 1;
        debug_assert!(
            rounds <= limit,
            "the pinned exclusions stopped growing late"
        );
    };
    ledger(root, input.me, &replay, &all)
}

/// Who `me` lets into its own view while it is not a member itself: the nodes it admitted.
fn own_admits(me: PeerId, replay: &Replay, all: &[Event]) -> BTreeSet<PeerId> {
    let outside = !replay.admitted.contains(&me);
    let own = all.iter().filter(|e| e.act == Act::Admit && e.author == me);
    own.filter(|_| outside).map(|e| e.subject).collect()
}

/// Admission records of administrators that did not make their subject a member.
fn requests(root: PeerId, replay: &Replay, all: &[Event]) -> BTreeMap<PeerId, Admission> {
    let open = |e: &&Event| {
        let s = e.subject;
        let known = replay.members.contains(&s) || replay.excluded.contains_key(&s);
        e.act == Act::Admit && replay.admins.contains(&e.author) && s != root && !known
    };
    let ask = || Admission {
        voters: Vec::new(),
        need: 1,
        label: None,
    };
    all.iter()
        .filter(open)
        .map(|e| (e.subject, ask()))
        .collect()
}

fn ledger(root: PeerId, me: PeerId, replay: &Replay, all: &[Event]) -> Ledger {
    let mut admitted = replay.admitted.clone();
    admitted.extend(own_admits(me, replay, all));
    Ledger {
        admitted,
        excluded: replay.excluded.clone(),
        pending: BTreeMap::new(),
        need: 1,
        admissions: requests(root, replay, all),
        forgets: BTreeMap::new(),
        admit_unix: BTreeMap::new(),
        forget_marks: BTreeMap::new(),
        forget_need: 1,
        admins: replay.admins.clone(),
        closed: replay.closed.clone(),
    }
}

#[cfg(test)]
#[path = "closure_tests.rs"]
mod tests;
