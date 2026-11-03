//! The list of open invites and the edit of an invite's "for whom" label.
//!
//! What it does: from the issue records and redemption marks of the network document builds the invites that
//! would still be granted (same verdict rules as redemption), and lets the admin who issued an invite rewrite its
//! label: the record is written again with the same version, time and issuer, only `for_whom` is new.
//! What it doesn't do: doesn't decide who may ask (the node checks admin rights) and doesn't revoke invites.
//! Links: `Ledger::open`/`relabel` are called by `node::node_ctl::control_invites`; the verdict is
//! `store::decide` / `store_admins::admin_verdict`, the label rule is `proof::issue_note`.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;

use super::proof::issue_note;
use super::store::{
    decide, issue_key, issue_record, used_key, IssueRecord, IssueVerdict, Ledger, ISSUE_PREFIX,
    USED_PREFIX,
};
use super::store_admins::{admin_verdict, by_admin, issuer_of, made_of};
use super::INVITE_TTL_SECS;
use crate::transport::net::{live, NetError, PeerId, ReplicaEntry};

/// An invite that can still be redeemed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenInvite {
    pub hash: String,
    pub for_whom: Option<String>,
    pub made_unix: u64,
    pub issuer: Option<PeerId>,
}

/// Outcome of `Ledger::relabel`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relabel {
    Done,
    /// No fresh unredeemed invite under this hash.
    NotFound,
    /// The invite is open, but it is not our own record (another admin's, or a holder's copy).
    NotIssuer,
}

/// Records of one kind grouped by the hash in their key.
fn by_hash(entries: &[ReplicaEntry], prefix: &str) -> BTreeMap<String, Vec<ReplicaEntry>> {
    let mut groups: BTreeMap<String, Vec<ReplicaEntry>> = BTreeMap::new();
    for e in entries.iter().filter(|e| !e.is_tombstone()) {
        let key = String::from_utf8_lossy(&e.key);
        if let Some(hash) = key.strip_prefix(prefix) {
            groups.entry(hash.to_string()).or_default().push(e.clone());
        }
    }
    groups
}

fn verdict_of(
    issued: &[ReplicaEntry],
    used: &[ReplicaEntry],
    admins: Option<&BTreeSet<PeerId>>,
    now_unix: u64,
) -> IssueVerdict {
    match admins {
        None => decide(&made_of(issued), !used.is_empty(), now_unix),
        Some(admins) => admin_verdict(issued, used, admins, now_unix),
    }
}

/// The newest fresh `made_unix` among readable records.
fn newest_fresh(issued: &[ReplicaEntry], now_unix: u64) -> Option<u64> {
    let fresh = |m: &u64| now_unix < m.saturating_add(INVITE_TTL_SECS);
    made_of(issued).into_iter().flatten().filter(fresh).max()
}

/// The record written by the issuer it names (a holder's copy names someone else).
fn issuers_own(issued: &[ReplicaEntry]) -> Option<(PeerId, &ReplicaEntry)> {
    issued.iter().find_map(|e| {
        let issuer = issuer_of(&e.value)?;
        (issuer == e.author).then_some((issuer, e))
    })
}

/// The records the shown fields come from: in a Managed network only the admins' ones, as in the verdict.
fn counted_records(
    issued: &[ReplicaEntry],
    admins: Option<&BTreeSet<PeerId>>,
) -> Vec<ReplicaEntry> {
    let counts = |e: &&ReplicaEntry| admins.is_none_or(|a| by_admin(e, a));
    issued.iter().filter(counts).cloned().collect()
}

fn open_row(
    hash: &str,
    issued: &[ReplicaEntry],
    used: &[ReplicaEntry],
    admins: Option<&BTreeSet<PeerId>>,
    now_unix: u64,
) -> Option<OpenInvite> {
    if verdict_of(issued, used, admins, now_unix) != IssueVerdict::Grant {
        return None;
    }
    let counted = counted_records(issued, admins);
    let note = issue_note(&counted);
    Some(OpenInvite {
        hash: hash.to_string(),
        for_whom: Some(note).filter(|n| !n.is_empty()),
        made_unix: newest_fresh(&counted, now_unix)?,
        issuer: issuers_own(&counted).map(|(issuer, _)| issuer),
    })
}

/// Open invites, newest first: those with the verdict `Grant`. `admins` is as in `Ledger::verdict`.
pub fn open_of(
    issued: &[ReplicaEntry],
    used: &[ReplicaEntry],
    admins: Option<&BTreeSet<PeerId>>,
    now_unix: u64,
) -> Vec<OpenInvite> {
    let used_by = by_hash(used, USED_PREFIX);
    let none = Vec::new();
    let mut rows: Vec<OpenInvite> = by_hash(issued, ISSUE_PREFIX)
        .iter()
        .filter_map(|(hash, recs)| {
            open_row(
                hash,
                recs,
                used_by.get(hash).unwrap_or(&none),
                admins,
                now_unix,
            )
        })
        .collect();
    rows.sort_by_key(|a| std::cmp::Reverse(a.made_unix));
    rows
}

impl Ledger {
    async fn scan_live(&self, prefix: &str) -> Result<Vec<ReplicaEntry>> {
        let all = self
            .replica
            .scan(prefix.as_bytes())
            .await
            .map_err(NetError::into_anyhow)?;
        Ok(live(all))
    }

    /// The invites that can still be redeemed, newest first.
    pub async fn open(
        &self,
        now_unix: u64,
        admins: Option<&BTreeSet<PeerId>>,
    ) -> Result<Vec<OpenInvite>> {
        let issued = self.scan_live(ISSUE_PREFIX).await?;
        let used = self.scan_live(USED_PREFIX).await?;
        Ok(open_of(&issued, &used, admins, now_unix))
    }

    /// Rewrites the label of our own open invite; `None` clears it. Time, version and issuer stay as they were.
    pub async fn relabel(
        &self,
        hash: &str,
        for_whom: Option<&str>,
        now_unix: u64,
        admins: Option<&BTreeSet<PeerId>>,
    ) -> Result<Relabel> {
        let issued = self.exact(issue_key(hash)).await?;
        let used = self.exact(used_key(hash)).await?;
        if verdict_of(&issued, &used, admins, now_unix) != IssueVerdict::Grant {
            return Ok(Relabel::NotFound);
        }
        let me = self.me();
        let own = issued
            .iter()
            .find(|e| e.author == me && issuer_of(&e.value) == Some(me));
        let Some(entry) = own else {
            return Ok(Relabel::NotIssuer);
        };
        let Some(made) = newest_fresh(std::slice::from_ref(entry), now_unix) else {
            return Ok(Relabel::NotFound);
        };
        let rec: IssueRecord = issue_record(made, for_whom);
        self.issue_as(hash, &rec, Some(me)).await?;
        Ok(Relabel::Done)
    }
}

#[cfg(test)]
#[path = "open_tests.rs"]
mod tests;
