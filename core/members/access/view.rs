//! The verdict view: a cheap shared handle through which the handshake, the connection registry and the
//! web layer ask "may this node be my neighbour".
//!
//! What it does: holds the latest published verdicts, answers `check`, tells which nodes were denied for
//! the first time on each publication, and keeps the last refusal per node for the status page.
//! What it doesn't do: no tally, no replica, no I/O; closing connections is `Members::cut`.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::members::access::tally::{How, Ledger};

pub use super::deny::Deny;
use crate::transport::net::PeerId;

/// Close code of a connection refused by the membership list.
pub const ACCESS_CLOSE_CODE: u32 = 403;
const MAX_REFUSALS: usize = 64;

/// Which side refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Me,
    Them,
}

/// The last refusal seen for a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub side: Side,
    pub deny: Deny,
    pub at_unix: u64,
}

impl Refusal {
    fn now(side: Side, deny: Deny) -> Refusal {
        Refusal {
            side,
            deny,
            at_unix: unix_now(),
        }
    }
}

/// Everything `check` needs, computed by the watcher.
#[derive(Clone, Debug)]
pub struct Verdicts {
    pub me: PeerId,
    pub ledger: Arc<Ledger>,
    pub my_votes: BTreeSet<PeerId>,
    pub blocks: BTreeSet<PeerId>,
    /// Per pending subject: the reason noted by the first voter, when it is fit to show.
    pub reasons: BTreeMap<PeerId, String>,
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl Verdicts {
    /// The decision for one node, in the fixed order of the contract.
    pub fn check(&self, peer: PeerId) -> Result<(), Deny> {
        if peer == self.me {
            return Ok(());
        }
        match self.first_deny(peer) {
            Some(deny) => Err(deny),
            None => self.check_listed(peer),
        }
    }

    /// The first refusal in contract order; my own forget votes are looked at last.
    fn first_deny(&self, peer: PeerId) -> Option<Deny> {
        self.early_deny(peer).or_else(|| {
            let mine = self.my_forget_subjects().any(|p| p == peer);
            mine.then_some(Deny::RemovedByMe)
        })
    }

    fn early_deny(&self, peer: PeerId) -> Option<Deny> {
        let l = &self.ledger;
        let table = [
            (l.how_excluded(&self.me) == Some(How::Left), Deny::Left),
            (l.excluded.contains_key(&peer), Deny::Excluded),
            (self.my_votes.contains(&peer), Deny::VotedByMe),
            (self.blocks.contains(&peer), Deny::Blocked),
            (l.is_forgotten(&peer), Deny::Forgotten),
        ];
        table.iter().find(|(hit, _)| *hit).map(|(_, deny)| *deny)
    }

    /// Subjects of live forget circles that I voted in, without myself.
    fn my_forget_subjects(&self) -> impl Iterator<Item = PeerId> + '_ {
        let me = self.me;
        let circles = self.ledger.forgets.iter();
        let mine = circles.filter(move |(_, round)| round.voters.contains(&me));
        mine.map(|(p, _)| *p).filter(move |p| *p != me)
    }

    /// Subjects of live forget circles that I voted in: hidden from me at once, before the circle completes.
    pub fn my_forgets(&self) -> BTreeSet<PeerId> {
        self.my_forget_subjects().collect()
    }

    /// Subjects hidden from this node: forgotten ones and those of its own pending forget votes.
    pub fn removed_here(&self) -> BTreeSet<PeerId> {
        let circles = self.ledger.forgets.iter();
        let forgotten = circles
            .filter(|(_, r)| r.at_unix.is_some())
            .map(|(p, _)| *p);
        let mut hidden: BTreeSet<PeerId> = self.my_forget_subjects().chain(forgotten).collect();
        hidden.remove(&self.me);
        hidden
    }

    fn check_listed(&self, peer: PeerId) -> Result<(), Deny> {
        match self.ledger.is_member(&peer) {
            true => Ok(()),
            false => Err(Deny::Unlisted),
        }
    }

    /// Nodes this view has an opinion about.
    pub(super) fn known(&self) -> BTreeSet<PeerId> {
        let l = &self.ledger;
        let listed = l.admitted.iter().chain(l.excluded.keys());
        let listed = listed.chain(&self.my_votes).chain(&self.blocks);
        listed
            .chain(l.forgets.keys())
            .filter(|p| **p != self.me)
            .copied()
            .collect()
    }
}

struct Inner {
    verdicts: Mutex<Option<Arc<Verdicts>>>,
    ready: tokio::sync::watch::Sender<bool>,
    refusals: Mutex<Vec<(PeerId, Refusal)>>,
    recompute: Arc<tokio::sync::Mutex<()>>,
    removed_noted: AtomicBool,
    allowed: tokio::sync::watch::Sender<u64>,
}

/// Shared handle to the latest verdicts. `open` lets everyone in; `pending` waits for the first
/// publication (until then it lets everyone in too, but `ready` does not return).
#[derive(Clone)]
pub struct AccessView {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for AccessView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccessView")
            .field("ready", &*self.inner.ready.borrow())
            .finish()
    }
}

impl Default for AccessView {
    fn default() -> Self {
        Self::open()
    }
}

impl AccessView {
    fn with_ready(ready: bool) -> Self {
        let (tx, _) = tokio::sync::watch::channel(ready);
        AccessView {
            inner: Arc::new(Inner {
                verdicts: Mutex::new(None),
                ready: tx,
                refusals: Mutex::new(Vec::new()),
                recompute: Arc::new(tokio::sync::Mutex::new(())),
                removed_noted: AtomicBool::new(false),
                allowed: tokio::sync::watch::channel(0).0,
            }),
        }
    }

    /// Lets everyone in; ready at once.
    pub fn open() -> Self {
        Self::with_ready(true)
    }

    /// Not ready until the first `publish`.
    pub fn pending() -> Self {
        Self::with_ready(false)
    }

    /// Returns once the view is ready (immediately for `open`).
    pub async fn ready(&self) {
        let mut rx = self.inner.ready.subscribe();
        let _ = rx.wait_for(|ready| *ready).await;
    }

    /// Replaces the verdicts; returns the nodes denied now that were not denied by the previous view.
    /// When the new view lets in a node the previous one denied, the `allowed_changes` counter grows.
    pub fn publish(&self, v: Verdicts) -> Vec<(PeerId, Deny)> {
        let new = Arc::new(v);
        let old = self.inner.verdicts.lock().unwrap().replace(new.clone());
        self.inner.ready.send_replace(true);
        self.drop_stale_refusals(&new);
        if !new.newly_allowed(old.as_deref()).is_empty() {
            self.inner.allowed.send_modify(|g| *g += 1);
        }
        new.newly_denied(old.as_deref())
    }

    /// A counter that grows each time a publication lets in a node the previous view denied.
    pub fn allowed_changes(&self) -> tokio::sync::watch::Receiver<u64> {
        self.inner.allowed.subscribe()
    }

    /// The decision for one node; everyone passes while no verdicts are published.
    pub fn check(&self, peer: PeerId) -> Result<(), Deny> {
        match self.verdicts() {
            Some(v) => v.check(peer),
            None => Ok(()),
        }
    }

    pub fn allows(&self, peer: PeerId) -> bool {
        self.check(peer).is_ok()
    }

    pub fn verdicts(&self) -> Option<Arc<Verdicts>> {
        self.inner.verdicts.lock().unwrap().clone()
    }

    /// Remembers the latest refusal for the node; at most 64 nodes are kept (the oldest goes).
    pub fn note_refusal(&self, peer: PeerId, side: Side, deny: Deny) {
        let mut list = self.inner.refusals.lock().unwrap();
        list.retain(|(p, _)| *p != peer);
        list.push((peer, Refusal::now(side, deny)));
        let extra = list.len().saturating_sub(MAX_REFUSALS);
        list.drain(..extra);
    }

    /// Forgets that a refusal was noted for the node.
    pub fn clear_refusal(&self, peer: PeerId) {
        self.retain_refusals(|(p, _)| *p != peer);
    }

    /// Drops my own refusals whose denial no longer holds (lifted or replaced by another reason).
    fn drop_stale_refusals(&self, new: &Verdicts) {
        self.retain_refusals(|(p, r)| r.side == Side::Them || new.check(*p) == Err(r.deny));
    }

    fn retain_refusals(&self, keep: impl FnMut(&(PeerId, Refusal)) -> bool) {
        self.inner.refusals.lock().unwrap().retain(keep);
    }

    pub fn refusals(&self) -> Vec<(PeerId, Refusal)> {
        self.inner.refusals.lock().unwrap().clone()
    }

    /// `true` only on the first call: the "removed from the list" line is logged once per run.
    pub fn first_removed_notice(&self) -> bool {
        !self.inner.removed_noted.swap(true, Ordering::Relaxed)
    }

    /// Serialises recomputations of the verdicts.
    pub fn recompute_lock(&self) -> Arc<tokio::sync::Mutex<()>> {
        self.inner.recompute.clone()
    }
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "view_forget_tests.rs"]
mod forget_tests;

#[cfg(test)]
#[path = "view_removed_here_tests.rs"]
mod removed_here_tests;
