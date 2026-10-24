//! Tests of the member actions (`actions.rs`): exclude, unexclude, block, unblock and leave, each against a
//! real in-memory shared replica and an own replica, with the verdicts recomputed by `Access`.
//! Nodes: `me` is the own node (seed 1), `a`, `b`, `c` are listed by me, `stranger` is not on the list.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use bytes::Bytes;

use crate::members::access::actions::{apply, ActionRefusal, MemberAction};
use crate::members::access::record::{
    admit_key, block_key, encode, exclude_key, parse_entry, AclKind, AclValue, Parsed, Via,
};
use crate::members::access::runtime::Access;
use crate::members::access::tally::{How, Ledger};
use crate::members::access::view::{AccessView, Deny};
use crate::members::membership::Members;
use crate::transport::net::contract::{pid, secret};
use crate::transport::net::mem::{MemReplica, MemSync, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_DOMAINS, REPLICA_NET};
use crate::transport::net::{
    BoxFuture, NetError, NetResult, PeerId, Replica, ReplicaEntry, ReplicaEvent, ReplicaId,
};

#[path = "actions_exclude_tests.rs"]
mod exclude_and_unexclude;

#[path = "actions_block_tests.rs"]
mod block_and_self_target;

#[path = "actions_leave_tests.rs"]
mod leave_and_failure;

#[path = "actions_reason_tests.rs"]
mod reason_and_withdrawal;

#[path = "actions_admit_tests.rs"]
mod admit_and_unadmit;

// --- setup and helpers ---

struct World {
    access: Access,
    shared: Arc<MemReplica>,
    own: Arc<MemReplica>,
    me: PeerId,
    fail_writes: Arc<AtomicBool>,
    _keep: (Arc<MemSyncNet>, Arc<MemSync>),
}

/// The node (seed 1) with `listed` admitted by it; the view starts pending, as in a real start.
fn setup(listed: &[PeerId]) -> World {
    build(listed, false)
}

fn setup_with_failing_shared(listed: &[PeerId]) -> World {
    build(listed, true)
}

fn build(listed: &[PeerId], failing: bool) -> World {
    let net = MemSyncNet::new();
    let node = secret(1);
    let ctx = SyncCtx {
        secret: node.clone(),
        network_seed: [9; 32],
        store_dir: None,
    };
    let backend = net.backend(ctx);
    let (shared, own) = (
        backend.replica_mem(REPLICA_NET).unwrap(),
        backend.replica_mem(REPLICA_DOMAINS).unwrap(),
    );
    let fail_writes = Arc::new(AtomicBool::new(false));
    let seen_by_access: Arc<dyn Replica> = match failing {
        true => Arc::new(FailingWrites {
            inner: shared.clone(),
            fail: fail_writes.clone(),
        }),
        false => shared.clone(),
    };
    let members = Members::with_access(AccessView::pending());
    let access = Access::from_parts(node.peer_id(), seen_by_access, own.clone(), members);
    let world = World {
        access,
        shared,
        own,
        me: node.peer_id(),
        fail_writes,
        _keep: (net, backend),
    };
    listed.iter().for_each(|p| world.given_admit(world.me, *p));
    world
}

impl World {
    /// An admit record of `author` for `subject` at a time long ago.
    fn given_admit(&self, author: PeerId, subject: PeerId) {
        self.given_record(author, admit_key(&subject), AclKind::Admit, subject, 1);
    }

    /// An exclusion vote of `voter` against `subject` at the given time.
    fn given_vote(&self, voter: PeerId, subject: PeerId, ts_micros: u64) {
        self.given_record(
            voter,
            exclude_key(&subject),
            AclKind::Exclude,
            subject,
            ts_micros,
        );
    }

    fn given_record(
        &self,
        author: PeerId,
        key: Vec<u8>,
        kind: AclKind,
        subject: PeerId,
        ts_micros: u64,
    ) {
        let value = AclValue {
            kind,
            subject,
            via: Via::Invite,
            made_unix: 111,
            note: String::new(),
        };
        let value = Bytes::from(encode(&value));
        self.shared.put_raw(ReplicaEntry {
            author,
            key,
            value,
            ts_micros,
        });
    }

    async fn apply(&self, act: MemberAction) -> Result<bool, ActionRefusal> {
        apply(&self.access, act).await
    }

    async fn refresh(&self) {
        self.access
            .refresh()
            .await
            .expect("the recompute must work on a healthy replica");
    }

    /// The decision of the published view for one node.
    fn verdict(&self, peer: PeerId) -> Result<(), Deny> {
        self.access.view.check(peer)
    }

    fn ledger(&self) -> Arc<Ledger> {
        self.access
            .view
            .verdicts()
            .expect("the view must be published after an action")
            .ledger
            .clone()
    }

    /// My entry under the key, a tombstone included.
    async fn mine(&self, replica: &Arc<MemReplica>, key: &[u8]) -> Option<ReplicaEntry> {
        let all = replica.scan(key).await.unwrap();
        all.into_iter()
            .find(|e| e.author == self.me && e.key == key)
    }

    /// Subjects of my live exclusion votes.
    async fn vote_subjects(&self) -> Vec<PeerId> {
        let all = self.shared.scan(b"acl/exclude/").await.unwrap();
        let mine = all
            .iter()
            .filter(|e| e.author == self.me && !e.is_tombstone());
        mine.filter_map(|e| match parse_entry(e) {
            Parsed::Record { value, .. } => Some(value.subject),
            _ => None,
        })
        .collect()
    }

    async fn shared_entries(&self) -> Vec<ReplicaEntry> {
        self.shared.scan(b"").await.unwrap()
    }

    async fn own_entries(&self) -> Vec<ReplicaEntry> {
        self.own.scan(b"").await.unwrap()
    }
}

/// A replica that delegates to the in-memory one and can refuse to write.
struct FailingWrites {
    inner: Arc<MemReplica>,
    fail: Arc<AtomicBool>,
}

impl Replica for FailingWrites {
    fn id(&self) -> ReplicaId {
        self.inner.id()
    }

    fn me(&self) -> PeerId {
        self.inner.me()
    }

    fn set_own(&self, key: &[u8], value: Bytes) -> BoxFuture<'_, NetResult<()>> {
        match self.fail.load(Ordering::SeqCst) {
            true => Box::pin(async { Err(NetError::Config("disk is full".into())) }),
            false => self.inner.set_own(key, value),
        }
    }

    fn del_own(&self, key: &[u8]) -> BoxFuture<'_, NetResult<()>> {
        match self.fail.load(Ordering::SeqCst) {
            true => Box::pin(async { Err(NetError::Config("disk is full".into())) }),
            false => self.inner.del_own(key),
        }
    }

    fn get(&self, author: PeerId, key: &[u8]) -> BoxFuture<'_, NetResult<Option<ReplicaEntry>>> {
        self.inner.get(author, key)
    }

    fn scan(&self, prefix: &[u8]) -> BoxFuture<'_, NetResult<Vec<ReplicaEntry>>> {
        self.inner.scan(prefix)
    }

    fn latest_per_key(&self, prefix: &[u8]) -> BoxFuture<'_, NetResult<Vec<ReplicaEntry>>> {
        self.inner.latest_per_key(prefix)
    }

    fn subscribe(&self) -> tokio::sync::broadcast::Receiver<ReplicaEvent> {
        self.inner.subscribe()
    }

    fn start_sync(&self) -> BoxFuture<'_, NetResult<()>> {
        self.inner.start_sync()
    }

    fn sync_with(&self, peer: PeerId) -> BoxFuture<'_, NetResult<()>> {
        self.inner.sync_with(peer)
    }
}
