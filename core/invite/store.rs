//! Storage of one-time invite issues in the network's shared document.
//!
//! What it does: writes the issue record `invite/<hash>`, the redemption mark `invite-used/<hash>` and the
//! redemption claim `invite-claim/<hash>`, judges from them the verdict on a presented secret and the winner
//! between two admitting nodes, and once an hour removes its own expired records.
//! What it doesn't do: doesn't touch the network (`MeshReplica` replicates the records) and doesn't store the secret itself —
//! only its hash.
//! Links: `invite::Issued::commit` writes, `invite::server` judges and redeems, the sweeper is started by
//! `node::start::serving`.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use bytes::Bytes;
use serde::{Deserialize, Serialize};

use super::INVITE_TTL_SECS;
use crate::node::output::say;
use crate::proxy::domains::DomainList;
use crate::transport::net::{live, NetError, PeerId, Replica, ReplicaEntry};
use crate::transport::net_mainline;

/// Key of the issue record; followed by the 64 hex characters of the secret's hash.
pub const ISSUE_PREFIX: &str = "invite/";
/// Key of the redemption mark; followed by the same hash.
pub const USED_PREFIX: &str = "invite-used/";
/// Key of the redemption claim; followed by the same hash.
pub const CLAIM_PREFIX: &str = "invite-claim/";
/// How many seconds an admitting node's claim counts as live: a crashed winner holds rivals back no longer than this.
pub const CLAIM_STALE_SECS: u64 = 30;
/// How many seconds the redemption mark lives: two invite lifetimes (a margin for clock skew).
pub const USED_KEEP_SECS: u64 = 2 * INVITE_TTL_SECS;
/// How often the sweeper walks over its own records.
pub const SWEEP_PERIOD: Duration = Duration::from_secs(3600);
/// How many characters of "for whom" stay in the record.
pub const FOR_WHOM_MAX_CHARS: usize = 64;

/// Issue record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueRecord {
    pub v: u8,
    pub made_unix: u64,
    pub for_whom: Option<String>,
}

/// Redemption mark.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsedRecord {
    pub v: u8,
    pub used_unix: u64,
}

/// An admitting node's claim to redeem a secret.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimRecord {
    pub v: u8,
    pub claimed_unix: u64,
}

/// Builds the issue record; "for whom" longer than `FOR_WHOM_MAX_CHARS` is cut by characters.
pub fn issue_record(made_unix: u64, for_whom: Option<&str>) -> IssueRecord {
    let cut = for_whom.map(|s| s.chars().take(FOR_WHOM_MAX_CHARS).collect::<String>());
    IssueRecord {
        v: 1,
        made_unix,
        for_whom: cut,
    }
}

/// Verdict on a presented secret.
#[derive(Debug, PartialEq)]
pub enum IssueVerdict {
    Grant,
    Used,
    Expired,
    Unknown,
    NotReady,
}

/// Pure verdict: a mark — `Used`; a fresh readable issue — `Grant`; an unreadable one — `NotReady`;
/// only expired ones — `Expired`; nothing — `Unknown`. `None` in `issued` — the issue couldn't be read.
pub fn decide(issued: &[Option<u64>], used: bool, now_unix: u64) -> IssueVerdict {
    if used {
        return IssueVerdict::Used;
    }
    let fresh = |m: &u64| now_unix < m.saturating_add(INVITE_TTL_SECS);
    if issued.iter().flatten().any(fresh) {
        return IssueVerdict::Grant;
    }
    if issued.iter().any(Option::is_none) {
        return IssueVerdict::NotReady;
    }
    if issued.is_empty() {
        return IssueVerdict::Unknown;
    }
    IssueVerdict::Expired
}

/// Outcome of comparing claims: this node admits or yields to a rival.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Settle {
    Win,
    Yield,
}

/// The one whose `PeerId` (by `as_bytes()`) is not smaller yields: the smallest wins.
pub fn settle(me: PeerId, rivals: &[PeerId]) -> Settle {
    if rivals.iter().any(|r| r.as_bytes() < me.as_bytes()) {
        return Settle::Yield;
    }
    Settle::Win
}

/// Live foreign claims: without our own, tombstones and ones older than `CLAIM_STALE_SECS` (a claim "from the future" is live).
pub fn live_rivals(entries: &[ReplicaEntry], me: PeerId, now_unix: u64) -> Vec<PeerId> {
    let fresh =
        |e: &&ReplicaEntry| now_unix.saturating_sub(e.ts_micros / 1_000_000) <= CLAIM_STALE_SECS;
    let theirs = |e: &&ReplicaEntry| e.author != me && !e.is_tombstone();
    entries
        .iter()
        .filter(theirs)
        .filter(fresh)
        .map(|e| e.author)
        .collect()
}

fn claim_key(hash: &str) -> Vec<u8> {
    format!("{CLAIM_PREFIX}{hash}").into_bytes()
}

fn issue_key(hash: &str) -> Vec<u8> {
    format!("{ISSUE_PREFIX}{hash}").into_bytes()
}

fn used_key(hash: &str) -> Vec<u8> {
    format!("{USED_PREFIX}{hash}").into_bytes()
}

/// Access to the invite records of the network's shared document.
#[derive(Clone)]
pub struct Ledger {
    replica: Arc<dyn Replica>,
}

impl Ledger {
    /// Log on the replica of the node's domain list.
    pub fn of(list: &DomainList) -> Self {
        Ledger {
            replica: list.shared_replica(),
        }
    }

    /// Writes the issue record under the secret's hash.
    pub async fn issue(&self, hash: &str, rec: &IssueRecord) -> Result<()> {
        let bytes = postcard::to_stdvec(rec)?;
        self.replica
            .set_own(&issue_key(hash), Bytes::from(bytes))
            .await
            .map_err(NetError::into_anyhow)
    }

    /// Writes the redemption mark under the secret's hash.
    pub async fn mark_used(&self, hash: &str, used_unix: u64) -> Result<()> {
        let bytes = postcard::to_stdvec(&UsedRecord { v: 1, used_unix })?;
        self.replica
            .set_own(&used_key(hash), Bytes::from(bytes))
            .await
            .map_err(NetError::into_anyhow)
    }

    /// Author of our own replica.
    pub fn me(&self) -> PeerId {
        self.replica.me()
    }

    /// Writes our own redemption claim under the secret's hash.
    pub async fn claim(&self, hash: &str, now_unix: u64) -> Result<()> {
        let bytes = postcard::to_stdvec(&ClaimRecord {
            v: 1,
            claimed_unix: now_unix,
        })?;
        self.replica
            .set_own(&claim_key(hash), Bytes::from(bytes))
            .await
            .map_err(NetError::into_anyhow)
    }

    /// Authors of live foreign claims on this hash.
    pub async fn rival_claims(&self, hash: &str, now_unix: u64) -> Result<Vec<PeerId>> {
        let entries = self.exact(claim_key(hash)).await?;
        Ok(live_rivals(&entries, self.replica.me(), now_unix))
    }

    /// Returns the verdict for the secret's hash; a document query error gives `NotReady`.
    pub async fn verdict(&self, hash: &str, now_unix: u64) -> IssueVerdict {
        let Ok(used) = self.exact(used_key(hash)).await else {
            return IssueVerdict::NotReady;
        };
        let Ok(issued) = self.exact(issue_key(hash)).await else {
            return IssueVerdict::NotReady;
        };
        let made: Vec<Option<u64>> = issued.iter().map(read_made).collect();
        decide(&made, !used.is_empty(), now_unix)
    }

    /// Removes its own expired records of the three groups; returns the number deleted.
    pub async fn sweep(&self, now_unix: u64) -> Result<usize> {
        let mut removed = 0;
        let groups = [
            (ISSUE_PREFIX, INVITE_TTL_SECS),
            (USED_PREFIX, USED_KEEP_SECS),
            (CLAIM_PREFIX, INVITE_TTL_SECS),
        ];
        for (prefix, keep) in groups {
            let all = self
                .replica
                .scan(prefix.as_bytes())
                .await
                .map_err(NetError::into_anyhow)?;
            for entry in live(all).iter().filter(|e| e.author == self.replica.me()) {
                removed += self.drop_if_old(entry, now_unix, keep).await?;
            }
        }
        Ok(removed)
    }

    async fn drop_if_old(&self, entry: &ReplicaEntry, now_unix: u64, keep: u64) -> Result<usize> {
        let written = entry.ts_micros / 1_000_000;
        if now_unix.saturating_sub(written) <= keep {
            return Ok(0);
        }
        self.replica
            .del_own(&entry.key)
            .await
            .map_err(NetError::into_anyhow)?;
        Ok(1)
    }

    /// Live records exactly under this key.
    async fn exact(&self, key: Vec<u8>) -> Result<Vec<ReplicaEntry>> {
        let all = self
            .replica
            .scan(&key)
            .await
            .map_err(NetError::into_anyhow)?;
        Ok(live(all).into_iter().filter(|e| e.key == key).collect())
    }
}

/// Returns `made_unix` from the issue record; empty or invalid bytes give `None`.
fn read_made(entry: &ReplicaEntry) -> Option<u64> {
    let rec: IssueRecord = postcard::from_bytes(&entry.value).ok()?;
    Some(rec.made_unix)
}

/// Sweeper: right away and every `SWEEP_PERIOD` removes its own expired records.
pub fn spawn_sweeper(ledger: Ledger) -> tokio::task::AbortHandle {
    tokio::spawn(async move {
        loop {
            report_sweep(ledger.sweep(net_mainline::unix_now()).await);
            tokio::time::sleep(SWEEP_PERIOD).await;
        }
    })
    .abort_handle()
}

fn report_sweep(result: Result<usize>) {
    match result {
        Ok(0) => {}
        Ok(n) => say("invite", format!("removed expired invite records: {n}")),
        Err(e) => say("invite", format!("invite record cleanup failed: {e:#}")),
    }
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "store_claim_tests.rs"]
mod claim_tests;
