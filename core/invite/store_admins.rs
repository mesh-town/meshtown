//! The Managed-network rule of the invite verdict: only records written by admins count.
//!
//! What it does: filters the issue and redemption-mark records of one secret by author and turns them
//! into a verdict; records with no admin author among the issues give `NotAdmin`.
//! What it doesn't do: no document access (`invite::store::Ledger` reads the records).
//! Links: called by `Ledger::verdict` when it gets the set of admins.

use std::collections::BTreeSet;

use super::store::{decide, IssueRecord, IssueVerdict};
use crate::transport::net::{PeerId, ReplicaEntry};

/// `made_unix` of each issue record; an empty or invalid value gives `None`.
pub(super) fn made_of(issued: &[ReplicaEntry]) -> Vec<Option<u64>> {
    issued.iter().map(read_made).collect()
}

fn read_made(entry: &ReplicaEntry) -> Option<u64> {
    let rec: IssueRecord = postcard::from_bytes(&entry.value).ok()?;
    Some(rec.made_unix)
}

/// The verdict from the records of `admins` alone; issues exist but none is from an admin — `NotAdmin`.
pub(super) fn admin_verdict(
    issued: &[ReplicaEntry],
    used: &[ReplicaEntry],
    admins: &BTreeSet<PeerId>,
    now_unix: u64,
) -> IssueVerdict {
    let by_admin = |e: &&ReplicaEntry| admins.contains(&e.author);
    let theirs: Vec<ReplicaEntry> = issued.iter().filter(by_admin).cloned().collect();
    let marked = used.iter().any(|e| admins.contains(&e.author));
    if !marked && theirs.is_empty() && !issued.is_empty() {
        return IssueVerdict::NotAdmin;
    }
    decide(&made_of(&theirs), marked, now_unix)
}
