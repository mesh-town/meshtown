//! Tests of the forget and unforget actions (`actions.rs`): a vote to drop a node that has been off
//! for good, and taking that vote back. Each test runs against real in-memory replicas with the
//! verdicts recomputed by `Access`. Nodes: `me` (seed 1) and the members `a` (2) and `b` (3) are on
//! the list, so three nodes are active and two votes are needed; in a network of two one vote is enough.

use std::sync::Arc;

use bytes::Bytes;

use super::*;
use crate::members::access::record::{encode, forget_key, key_for, parse_entry, Parsed};
use crate::members::access::tally::Ledger;
use crate::members::access::view::{AccessView, Deny};
use crate::members::membership::Members;
use crate::transport::net::contract::{pid, secret};
use crate::transport::net::mem::{MemReplica, MemSync, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_DOMAINS, REPLICA_NET};
use crate::transport::net::{Replica, ReplicaEntry};

/// A time (seconds) far ahead of any real clock.
const FUTURE: u64 = 4_000_000_000;

#[path = "actions_forget_vote_tests.rs"]
mod forget_and_refusals;

#[path = "actions_forget_again_tests.rs"]
mod forget_again_and_unforget;

// --- setup and helpers ---

/// A node of the in-memory network with its replicas.
struct Node {
    access: Access,
    shared: Arc<MemReplica>,
    me: PeerId,
    net: Arc<MemSyncNet>,
    _backend: Arc<MemSync>,
}

/// The node `me` (seed 1) with `listed` admitted by it; the view starts pending, as in a real start.
fn setup(listed: &[PeerId]) -> Node {
    let node = Node::new(&MemSyncNet::new(), 1);
    listed
        .iter()
        .for_each(|p| node.given_admit(node.me, *p, 111, 1));
    node
}

impl Node {
    fn new(net: &Arc<MemSyncNet>, seed: u8) -> Node {
        let secret = secret(seed);
        let ctx = SyncCtx {
            secret: secret.clone(),
            network_seed: [9; 32],
            store_dir: None,
        };
        let backend = net.backend(ctx);
        let (shared, own) = (
            backend.replica_mem(REPLICA_NET).unwrap(),
            backend.replica_mem(REPLICA_DOMAINS).unwrap(),
        );
        let members = Members::with_access(AccessView::pending());
        let access = Access::from_parts(secret.peer_id(), shared.clone(), own, members);
        Node {
            access,
            shared,
            me: secret.peer_id(),
            net: net.clone(),
            _backend: backend,
        }
    }

    /// Another node of the same network with its own replicas; it admitted `listed`.
    fn peer(&self, seed: u8, listed: &[PeerId]) -> Node {
        let node = Node::new(&self.net, seed);
        listed
            .iter()
            .for_each(|p| node.given_admit(node.me, *p, 111, 1));
        node
    }

    fn given_admit(&self, author: PeerId, subject: PeerId, made_unix: u64, ts_micros: u64) {
        self.given(author, AclKind::Admit, subject, made_unix, ts_micros);
    }

    fn given(&self, author: PeerId, kind: AclKind, subject: PeerId, made_unix: u64, ts: u64) {
        let value = AclValue {
            kind,
            subject,
            via: Via::Invite,
            made_unix,
            note: String::new(),
        };
        self.shared.put_raw(ReplicaEntry {
            author,
            key: key_for(kind, &subject),
            value: Bytes::from(encode(&value)),
            ts_micros: ts,
        });
    }

    /// The records of `other` reach this node (the sync that the real replicas do).
    async fn pull_from(&self, other: &Node) {
        for entry in other.shared.scan(b"").await.unwrap() {
            self.shared.put_raw(entry);
        }
    }

    fn go_online(&self, peer: PeerId) {
        self.access.members.add_verified_conn(peer);
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

    /// My live forget record about `subject`, if any.
    async fn my_forget(&self, subject: PeerId) -> Option<AclValue> {
        let entry = self
            .shared
            .get(self.me, &forget_key(&subject))
            .await
            .unwrap();
        match parse_entry(&entry.filter(|e| !e.is_tombstone())?) {
            Parsed::Record { value, .. } => Some(value),
            other => panic!("the forget vote must be a record, got {other:?}"),
        }
    }

    async fn entries(&self) -> Vec<ReplicaEntry> {
        self.shared.scan(b"").await.unwrap()
    }
}
