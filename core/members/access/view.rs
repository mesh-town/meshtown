//! The verdict view: a cheap shared handle through which the handshake, the connection registry and the
//! web layer ask "may this node be my neighbour".
//!
//! What it does: holds the latest published verdicts, answers `check`, tells which nodes were denied for
//! the first time on each publication, and keeps the last refusal per node for the status page.
//! What it doesn't do: no tally, no replica, no I/O; closing connections is `Members::cut`.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::members::access::tally::Ledger;
use crate::transport::net::PeerId;

/// Close code of a connection refused by the membership list.
pub const ACCESS_CLOSE_CODE: u32 = 403;
const MAX_REFUSALS: usize = 64;
const WIRE_PREFIX: &str = "code 403: ";

/// Why a node is not let in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deny {
    Left,
    Excluded,
    VotedByMe,
    Blocked,
    Unlisted,
    /// Only from the wire: the other side refused by its own vote or block.
    Refused,
}

impl Deny {
    /// The reason sent in the close frame.
    pub fn wire_reason(&self) -> &'static [u8] {
        match self {
            Deny::Left => b"left",
            Deny::Excluded => b"excluded",
            Deny::VotedByMe | Deny::Blocked | Deny::Refused => b"refused",
            Deny::Unlisted => b"not admitted",
        }
    }

    /// The word used in the JSON contract.
    pub fn code(&self) -> &'static str {
        match self {
            Deny::Left => "left",
            Deny::Excluded => "excluded",
            Deny::VotedByMe => "voted_by_me",
            Deny::Blocked => "blocked",
            Deny::Unlisted => "unlisted",
            Deny::Refused => "refused",
        }
    }

    /// Parses `code 403: <reason>` as reported by `PeerConn::close_reason`.
    pub fn from_wire(text: &str) -> Option<Deny> {
        match text.strip_prefix(WIRE_PREFIX)? {
            "left" => Some(Deny::Left),
            "excluded" => Some(Deny::Excluded),
            "refused" => Some(Deny::Refused),
            "not admitted" => Some(Deny::Unlisted),
            _ => None,
        }
    }
}

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
        let denies = [
            (self.ledger.excluded.contains_key(&self.me), Deny::Left),
            (self.ledger.excluded.contains_key(&peer), Deny::Excluded),
            (self.my_votes.contains(&peer), Deny::VotedByMe),
            (self.blocks.contains(&peer), Deny::Blocked),
        ];
        match denies.iter().find(|(hit, _)| *hit) {
            Some((_, deny)) => Err(*deny),
            None => self.check_listed(peer),
        }
    }

    fn check_listed(&self, peer: PeerId) -> Result<(), Deny> {
        match self.ledger.is_member(&peer) {
            true => Ok(()),
            false => Err(Deny::Unlisted),
        }
    }

    /// Nodes this view has an opinion about.
    fn known(&self) -> BTreeSet<PeerId> {
        let mut all: BTreeSet<PeerId> = self.ledger.admitted.clone();
        all.extend(self.ledger.excluded.keys().copied());
        all.extend(self.my_votes.iter().copied());
        all.extend(self.blocks.iter().copied());
        all.remove(&self.me);
        all
    }
}

struct Inner {
    verdicts: Mutex<Option<Arc<Verdicts>>>,
    ready: tokio::sync::watch::Sender<bool>,
    refusals: Mutex<Vec<(PeerId, Refusal)>>,
    recompute: Arc<tokio::sync::Mutex<()>>,
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
    pub fn publish(&self, v: Verdicts) -> Vec<(PeerId, Deny)> {
        let new = Arc::new(v);
        let old = self.inner.verdicts.lock().unwrap().replace(new.clone());
        self.inner.ready.send_replace(true);
        self.drop_stale_refusals(&new);
        let mut candidates = new.known();
        candidates.extend(old.iter().flat_map(|o| o.known()));
        candidates
            .into_iter()
            .filter(|p| old.as_ref().is_none_or(|o| o.check(*p).is_ok()))
            .filter_map(|p| new.check(p).err().map(|deny| (p, deny)))
            .collect()
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
        list.push((
            peer,
            Refusal {
                side,
                deny,
                at_unix: unix_now(),
            },
        ));
        let extra = list.len().saturating_sub(MAX_REFUSALS);
        list.drain(..extra);
    }

    /// Forgets that a refusal was noted for the node.
    pub fn clear_refusal(&self, peer: PeerId) {
        self.inner
            .refusals
            .lock()
            .unwrap()
            .retain(|(p, _)| *p != peer);
    }

    /// Drops my own refusals whose denial no longer holds (lifted or replaced by another reason).
    fn drop_stale_refusals(&self, new: &Verdicts) {
        let holds =
            |peer: &PeerId, r: &Refusal| r.side == Side::Them || new.check(*peer) == Err(r.deny);
        self.inner
            .refusals
            .lock()
            .unwrap()
            .retain(|(p, r)| holds(p, r));
    }

    pub fn refusals(&self) -> Vec<(PeerId, Refusal)> {
        self.inner.refusals.lock().unwrap().clone()
    }

    /// Serialises recomputations of the verdicts.
    pub fn recompute_lock(&self) -> Arc<tokio::sync::Mutex<()>> {
        self.inner.recompute.clone()
    }
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
