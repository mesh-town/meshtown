//! The Managed-network rule of the invite verdict: only records written by admins count.
//!
//! What it does: filters the issue and redemption-mark records of one secret by author and turns them
//! into a verdict; an issue counts only if an admin wrote it on behalf of an admin, otherwise `NotAdmin`.
//! Value layout of an issue record: v1 is `postcard(IssueRecord)`; v2 is the same with `v = 2` followed by the
//! 32-byte id of the issuer. Readers of the plain record ignore the tail.
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

/// Version of an issue record that carries the issuer's id after the record.
pub(super) const ISSUE_V_ISSUER: u8 = 2;

/// The stored value of an issue record. `None` gives the plain v1 encoding; `Some(issuer)` gives the record
/// with version `ISSUE_V_ISSUER` followed by the 32 bytes of the issuer's id (older readers drop that tail).
pub(super) fn issue_value(rec: &IssueRecord, issuer: Option<PeerId>) -> postcard::Result<Vec<u8>> {
    let Some(issuer) = issuer else {
        return postcard::to_stdvec(rec);
    };
    let tagged = IssueRecord {
        v: ISSUE_V_ISSUER,
        ..rec.clone()
    };
    let mut bytes = postcard::to_stdvec(&tagged)?;
    bytes.extend(postcard::to_stdvec(&issuer)?);
    Ok(bytes)
}

/// The issuer named by a v2 issue record; a v1 record, another version or an unreadable value gives `None`.
pub(super) fn issuer_of(value: &[u8]) -> Option<PeerId> {
    let (rec, tail) = postcard::take_from_bytes::<IssueRecord>(value).ok()?;
    if rec.v != ISSUE_V_ISSUER {
        return None;
    }
    postcard::from_bytes::<PeerId>(tail).ok()
}

/// Whether an issue record counts in a Managed network: an admin wrote it on behalf of an admin.
fn by_admin(entry: &ReplicaEntry, admins: &BTreeSet<PeerId>) -> bool {
    admins.contains(&entry.author)
        && issuer_of(&entry.value).is_some_and(|issuer| admins.contains(&issuer))
}

/// The verdict from the records of `admins` alone. An issue record counts only if an admin wrote it and it
/// names an admin as the issuer (v2); v1 records do not count. Issues exist but none counts — `NotAdmin`.
pub(super) fn admin_verdict(
    issued: &[ReplicaEntry],
    used: &[ReplicaEntry],
    admins: &BTreeSet<PeerId>,
    now_unix: u64,
) -> IssueVerdict {
    let theirs: Vec<ReplicaEntry> = issued
        .iter()
        .filter(|e| by_admin(e, admins))
        .cloned()
        .collect();
    let marked = used.iter().any(|e| admins.contains(&e.author));
    if !marked && theirs.is_empty() && !issued.is_empty() {
        return IssueVerdict::NotAdmin;
    }
    decide(&made_of(&theirs), marked, now_unix)
}
