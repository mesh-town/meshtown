//! Shared setup of the administered-network action tests: the seats, the world and its helpers.

use std::sync::Arc;

use bytes::Bytes;

use crate::members::access::actions::{apply, ActionRefusal, MemberAction};
use crate::members::access::record::{encode, key_for, AclKind, AclValue, Via};
use crate::members::access::runtime::Access;
use crate::members::access::tally::Ledger;
use crate::members::access::view::{AccessView, Deny};
use crate::members::membership::Members;
use crate::transport::net::contract::{pid, secret};
use crate::transport::net::mem::{MemReplica, MemSync, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_DOMAINS, REPLICA_NET};
use crate::transport::net::{PeerId, Replica, ReplicaEntry};

/// Where `me` stands in the administered network.
#[derive(Clone, Copy)]
pub(super) enum Seat {
    /// The creator and the only administrator.
    Root,
    /// Granted the role by the root, who is another administrator.
    Admin,
    /// Admitted by the root, no role.
    Member,
}

/// The member that every scene starts with.
pub(super) fn member() -> PeerId {
    pid(3)
}

pub(super) fn other_admin() -> PeerId {
    pid(4)
}

pub(super) fn outsider() -> PeerId {
    pid(9)
}

pub(super) struct World {
    pub(super) access: Access,
    pub(super) shared: Arc<MemReplica>,
    pub(super) me: PeerId,
    pub(super) root: PeerId,
    _keep: (Arc<MemSyncNet>, Arc<MemSync>),
}

/// An administered network with the member `x`; `seat` tells who `me` is.
pub(super) fn setup(seat: Seat) -> World {
    let me = secret(1).peer_id();
    let root = match seat {
        Seat::Root => me,
        _ => pid(2),
    };
    let world = build(root);
    world.given_admit(1, root, member());
    match seat {
        Seat::Root => {}
        Seat::Admin => {
            world.given_admit(1, root, me);
            world.given_grant(2, root, me);
        }
        Seat::Member => world.given_admit(1, root, me),
    }
    world
}

pub(super) fn build(root: PeerId) -> World {
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
    let members = Members::with_access(AccessView::pending());
    let access = Access::from_parts(node.peer_id(), shared.clone(), own, members, root);
    World {
        access,
        shared,
        me: node.peer_id(),
        root,
        _keep: (net, backend),
    }
}

impl World {
    /// An admission written by `author` at second `at`.
    pub(super) fn given_admit(&self, at: u64, author: PeerId, subject: PeerId) {
        self.given_record(at, author, AclKind::Admit, subject);
    }

    pub(super) fn given_grant(&self, at: u64, author: PeerId, subject: PeerId) {
        self.given_record(at, author, AclKind::Grant, subject);
    }

    /// An exclusion of `author` against `subject` at second `at`.
    pub(super) fn given_exclude(&self, at: u64, author: PeerId, subject: PeerId) {
        self.given_record(at, author, AclKind::Exclude, subject);
    }

    /// `who` leaves: an exclusion of itself.
    pub(super) fn given_leave(&self, at: u64, who: PeerId) {
        self.given_exclude(at, who, who);
    }

    /// `node` is admitted and made an administrator by the root.
    pub(super) fn given_administrator(&self, node: PeerId) {
        self.given_admit(1, self.root, node);
        self.given_grant(2, self.root, node);
    }

    pub(super) fn given_record(&self, at: u64, author: PeerId, kind: AclKind, subject: PeerId) {
        let value = AclValue {
            kind,
            subject,
            via: Via::Invite,
            made_unix: at,
            note: String::new(),
        };
        self.shared.put_raw(ReplicaEntry {
            author,
            key: key_for(kind, &subject),
            value: Bytes::from(encode(&value)),
            ts_micros: at * 1_000_000,
        });
    }

    pub(super) async fn apply(&self, act: MemberAction) -> Result<bool, ActionRefusal> {
        apply(&self.access, act).await
    }

    pub(super) async fn refresh(&self) {
        self.access
            .refresh()
            .await
            .expect("the recompute must work on a healthy replica");
    }

    /// What this node's view says about letting `peer` in.
    pub(super) fn verdict(&self, peer: PeerId) -> Result<(), Deny> {
        self.access.view.check(peer)
    }

    pub(super) fn ledger(&self) -> Arc<Ledger> {
        self.access
            .view
            .verdicts()
            .expect("the view must be published after an action")
            .ledger
            .clone()
    }

    /// The time of my live record under the key.
    pub(super) async fn live_ts(&self, key: &[u8]) -> Option<u64> {
        self.live_of(self.me, key).await.map(|e| e.ts_micros)
    }

    /// The live record of `author` under exactly this key.
    pub(super) async fn live_of(&self, author: PeerId, key: &[u8]) -> Option<ReplicaEntry> {
        let all = self.shared.scan(key).await.unwrap();
        all.into_iter()
            .find(|e| e.author == author && e.key == key && !e.is_tombstone())
    }

    pub(super) async fn shared_entries(&self) -> Vec<ReplicaEntry> {
        self.shared.scan(b"").await.unwrap()
    }
}
