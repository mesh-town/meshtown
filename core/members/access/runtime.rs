//! Recompute of the membership verdicts: scan the list records, tally, publish, cut the newly denied
//! connections and write the own "admitted" records for connected neighbours.
//!
//! What it does: `Access::refresh` is the one place the verdicts are made.
//! What it doesn't do: no timers or subscriptions (`watch`), no user actions (`actions`).

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::sync::watch;

use crate::members::membership::Members;
use crate::node::output::say;
use crate::proxy::domains::DomainList;
use crate::transport::net::{NetResult, PeerId, Replica, ReplicaEntry};
use crate::transport::net_mainline::unix_now;

use super::reasons::label_admissions;
use super::record::{self, tally_input, AclKind, AclValue, Via, ACL_PREFIX, BLOCK_PREFIX};
use super::subjects::live_subjects;
use super::tally::{tally, Closed};
use super::view::{AccessView, Verdicts};

/// The state a recompute needs; cheap to share behind an `Arc`.
pub struct Access {
    pub(crate) view: AccessView,
    pub(crate) shared: Arc<dyn Replica>,
    pub(crate) own: Arc<dyn Replica>,
    pub(crate) me: PeerId,
    pub(crate) members: Members,
    pub(crate) root: PeerId,
    ignored_logged: AtomicUsize,
    /// The closing of the network by an administrator, as the last recompute saw it.
    closed_tx: watch::Sender<Option<Closed>>,
}

/// The records a recompute reads.
struct Scan {
    acl: Vec<ReplicaEntry>,
    blocks: Vec<ReplicaEntry>,
}

impl Access {
    pub fn new(list: &DomainList, members: &Members, root: PeerId) -> Access {
        Access::from_parts(
            list.me(),
            list.shared_replica(),
            list.own_replica(),
            members.clone(),
            root,
        )
    }

    pub fn from_parts(
        me: PeerId,
        shared: Arc<dyn Replica>,
        own: Arc<dyn Replica>,
        members: Members,
        root: PeerId,
    ) -> Access {
        Access {
            view: members.access(),
            shared,
            own,
            me,
            members,
            root,
            ignored_logged: AtomicUsize::new(0),
            closed_tx: watch::channel(None).0,
        }
    }

    /// The closing of the network: `Some` once an administrator's close is counted. It changes only
    /// after the verdicts that carry it are published.
    pub fn closed(&self) -> watch::Receiver<Option<Closed>> {
        self.closed_tx.subscribe()
    }

    /// Recomputes and publishes the verdicts, then writes the missing witnesses. A failed scan leaves
    /// the previous view in place.
    pub async fn refresh(&self) -> NetResult<Arc<Verdicts>> {
        let lock = self.view.recompute_lock();
        let _guard = lock.lock().await;
        let verdicts = self.recompute().await?;
        match self.write_witnesses(&verdicts).await? {
            0 => Ok(verdicts),
            _ => self.recompute().await,
        }
    }

    async fn scan(&self) -> NetResult<Scan> {
        let acl = self.shared.scan(ACL_PREFIX).await?;
        let blocks = self.own.scan(BLOCK_PREFIX).await?;
        Ok(Scan { acl, blocks })
    }

    async fn recompute(&self) -> NetResult<Arc<Verdicts>> {
        let scan = self.scan().await?;
        let (input, ignored) = tally_input(self.root, self.me, &scan.acl);
        self.log_ignored(ignored);
        let blocks = live_subjects(&scan.blocks, self.me, AclKind::Block);
        let mut ledger = tally(&input);
        label_admissions(&mut ledger, &scan.acl);
        let closed = ledger.closed.clone();
        let verdicts = Verdicts {
            me: self.me,
            ledger: Arc::new(ledger),
            blocks,
        };
        let denied = self.view.publish(verdicts.clone());
        self.closed_tx.send_if_modified(|seen| {
            let changed = *seen != closed;
            *seen = closed;
            changed
        });
        self.cut_denied(denied.into_iter().map(|(p, _)| p).collect());
        Ok(Arc::new(verdicts))
    }

    /// One log line when the number of unreadable records changes.
    fn log_ignored(&self, ignored: usize) {
        if self.ignored_logged.swap(ignored, Ordering::Relaxed) == ignored || ignored == 0 {
            return;
        }
        say(
            "access",
            format!("acl: {ignored} list records ignored (unreadable or foreign)"),
        );
    }

    /// Closes the connections of the newly denied nodes and of every connected node that is denied now.
    fn cut_denied(&self, newly: Vec<PeerId>) {
        let mut targets: BTreeSet<PeerId> = newly.into_iter().collect();
        targets.extend(
            self.members
                .ids()
                .into_iter()
                .filter(|p| self.view.check(*p).is_err()),
        );
        for peer in targets {
            self.cut_one(peer);
        }
    }

    fn cut_one(&self, peer: PeerId) {
        let Err(deny) = self.view.check(peer) else {
            return;
        };
        let closed = self.members.cut(peer, deny);
        if closed > 0 {
            say(
                "access",
                format!(
                    "acl: closed {closed} connection(s) of a denied node ({})",
                    deny.code()
                ),
            );
        }
    }

    /// A witness record for each connected node the list lets in and I have not admitted yet.
    async fn write_witnesses(&self, v: &Verdicts) -> NetResult<usize> {
        let mut written = 0;
        for peer in self.members.ids() {
            written += self.witness_one(v, peer).await?;
        }
        Ok(written)
    }

    async fn witness_one(&self, v: &Verdicts, peer: PeerId) -> NetResult<usize> {
        if peer == self.me || v.check(peer).is_err() {
            return Ok(0);
        }
        let admit = AclValue {
            kind: AclKind::Admit,
            subject: peer,
            via: Via::Witness,
            made_unix: unix_now(),
            note: String::new(),
        };
        Ok(usize::from(record::ensure(&*self.shared, &admit).await?))
    }
}

#[cfg(test)]
#[path = "runtime_closed_tests.rs"]
mod closed_tests;
