//! Reading the own live records of the list out of a scan.

use std::collections::{BTreeMap, BTreeSet};

use crate::transport::net::{PeerId, ReplicaEntry};

use super::authority::Rules;
use super::record::{parse_entry, parse_key, AclKind, Parsed, Via};
use super::tally::Exclusion;

/// Subjects of the live records of `kind` written by `me` in `entries`.
pub(super) fn live_subjects(
    entries: &[ReplicaEntry],
    me: PeerId,
    kind: AclKind,
) -> BTreeSet<PeerId> {
    let subject = |e: &ReplicaEntry| match parse_entry(e) {
        Parsed::Record { author, value, .. } if author == me && value.kind == kind => {
            Some(value.subject)
        }
        _ => None,
    };
    entries.iter().filter_map(subject).collect()
}

/// Subjects `me` currently votes to exclude, without `me` itself. Under managed
/// rules a vote counts only while the tally still excludes its subject: a later
/// admit by an admin lifts the exclusion and the vote stops refusing.
pub(super) fn my_votes_under(
    rules: Rules,
    entries: &[ReplicaEntry],
    me: PeerId,
    excluded: &BTreeMap<PeerId, Exclusion>,
) -> BTreeSet<PeerId> {
    let mut votes = live_subjects(entries, me, AclKind::Exclude);
    votes.remove(&me);
    if rules.is_managed() {
        votes.retain(|p| excluded.contains_key(p));
    }
    votes
}

/// Subjects of the live admits of `me` that are only witnesses.
pub(super) fn witnessed_subjects(entries: &[ReplicaEntry], me: PeerId) -> BTreeSet<PeerId> {
    let subject = |e: &ReplicaEntry| match parse_entry(e) {
        Parsed::Record { author, value, .. }
            if author == me && value.kind == AclKind::Admit && value.via == Via::Witness =>
        {
            Some(value.subject)
        }
        _ => None,
    };
    entries.iter().filter_map(subject).collect()
}

/// Subjects of the tombstones of `kind` written by `me`: records I took back.
pub(super) fn withdrawn_subjects(
    entries: &[ReplicaEntry],
    me: PeerId,
    kind: AclKind,
) -> BTreeSet<PeerId> {
    let subject = |e: &ReplicaEntry| match parse_entry(e) {
        Parsed::Withdrawn { author, key } if author == me => match parse_key(&key) {
            Some((k, Some(s))) if k == kind => Some(s),
            _ => None,
        },
        _ => None,
    };
    entries.iter().filter_map(subject).collect()
}

#[cfg(test)]
#[path = "subjects_tests.rs"]
mod tests;
