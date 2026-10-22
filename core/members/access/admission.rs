//! Admission count: who is let in by one confirmation, and who is still waiting.
//!
//! What it does: finds the set `T` of admitted nodes as the least fixed point of "enough distinct
//! unblocked authors from `T` vouch for the subject" and lists the subjects that have a vouching record
//! but fall short of the threshold. The result does not depend on the order of the input.
//! What it doesn't do: no exclusion votes (the pass in `tally` owns them), no replica, no clock.

use std::collections::{BTreeMap, BTreeSet};

use crate::transport::net::PeerId;

use super::record::Via;
use super::tally::{Admission, AdmitIn, Exclusion};

type Excluded = BTreeMap<PeerId, Exclusion>;

/// The admitted set and the requests that have not reached the threshold.
pub(super) struct Admitted {
    pub set: BTreeSet<PeerId>,
    pub short: BTreeMap<PeerId, Admission>,
}

/// A record of this kind is a vote for the subject at nodes other than its author.
pub(super) fn counts_elsewhere(via: Via) -> bool {
    matches!(via, Via::None | Via::Invite | Via::Migration)
}

/// Admit records with self-admits dropped and repeats of one (author, subject) folded into one.
struct Rows {
    /// subject -> author -> the author's vote counts at other nodes.
    by_subject: BTreeMap<PeerId, BTreeMap<PeerId, bool>>,
    by_author: BTreeMap<PeerId, BTreeSet<PeerId>>,
}

fn rows(admits: &[AdmitIn]) -> Rows {
    let mut out = Rows {
        by_subject: BTreeMap::new(),
        by_author: BTreeMap::new(),
    };
    for a in admits.iter().filter(|a| a.author != a.subject) {
        let counts = out
            .by_subject
            .entry(a.subject)
            .or_default()
            .entry(a.author)
            .or_insert(false);
        *counts |= counts_elsewhere(a.via);
        out.by_author.entry(a.author).or_default().insert(a.subject);
    }
    out
}

/// An author whose records are read: `me`, or anybody not excluded.
fn open(me: PeerId, excluded: &Excluded, author: &PeerId) -> bool {
    *author == me || !excluded.contains_key(author)
}

/// Confirmations that admit a newcomer, whatever the size of the network. Exclusion counts its own
/// threshold in `tally::need_for`.
const ADMIT_NEED: u8 = 1;

/// Authors from `set` whose vote for `x` counts, ascending.
fn voters(
    me: PeerId,
    rows: &Rows,
    set: &BTreeSet<PeerId>,
    excluded: &Excluded,
    x: &PeerId,
) -> Vec<PeerId> {
    let Some(authors) = rows.by_subject.get(x) else {
        return Vec::new();
    };
    let counted =
        |(a, counts): &(&PeerId, &bool)| **counts && set.contains(*a) && open(me, excluded, a);
    authors.iter().filter(counted).map(|(a, _)| *a).collect()
}

/// Rounds until nothing is added: a subject enters when its votes reach `ADMIT_NEED`.
fn grow(me: PeerId, rows: &Rows, excluded: &Excluded) -> BTreeSet<PeerId> {
    let mut set = BTreeSet::from([me]);
    set.extend(rows.by_author.get(&me).into_iter().flatten().copied());
    loop {
        let enters = |x: &&PeerId| {
            !set.contains(*x)
                && voters(me, rows, &set, excluded, x).len() >= usize::from(ADMIT_NEED)
        };
        let added: Vec<PeerId> = rows.by_subject.keys().filter(enters).copied().collect();
        if added.is_empty() {
            return set;
        }
        set.extend(added);
    }
}

/// A record on `x` by an open author from the set, whatever its kind.
fn has_backer(
    me: PeerId,
    rows: &Rows,
    set: &BTreeSet<PeerId>,
    excluded: &Excluded,
    x: &PeerId,
) -> bool {
    let authors = rows.by_subject.get(x).into_iter().flatten();
    authors
        .into_iter()
        .any(|(a, _)| set.contains(a) && open(me, excluded, a))
}

/// Subjects with a backer and too few votes: outside `set`, held only by my own vote, or me.
fn requests(
    me: PeerId,
    rows: &Rows,
    set: &BTreeSet<PeerId>,
    excluded: &Excluded,
) -> BTreeMap<PeerId, Admission> {
    let mine_counts = |x: &PeerId| {
        rows.by_subject
            .get(x)
            .and_then(|a| a.get(&me))
            .copied()
            .unwrap_or(false)
    };
    let mut out = BTreeMap::new();
    for x in rows.by_subject.keys().chain([&me]) {
        let inside = set.contains(x);
        let relevant = !inside || *x == me || mine_counts(x);
        let need = ADMIT_NEED;
        let found = voters(me, rows, set, excluded, x);
        if relevant
            && !excluded.contains_key(x)
            && has_backer(me, rows, set, excluded, x)
            && found.len() < usize::from(need)
        {
            out.insert(
                *x,
                Admission {
                    voters: found,
                    need,
                    label: None,
                },
            );
        }
    }
    out
}

/// The admitted set `T` and the requests short of the threshold. Empty input gives `{me}` and no requests.
pub(super) fn admit_closure(me: PeerId, admits: &[AdmitIn], excluded: &Excluded) -> Admitted {
    let rows = rows(admits);
    let set = grow(me, &rows, excluded);
    let short = requests(me, &rows, &set, excluded);
    Admitted { set, short }
}

#[cfg(test)]
#[path = "admission_tests.rs"]
mod tests;
