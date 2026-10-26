//! Tests of the `closed` channel of `Access` (`runtime.rs`): it follows the closing of the network that the
//! recompute finds, and stays empty when the closing does not count.
//! Nodes: `me` (seed 1, a plain member), the root (2, the administrator) and the member `x` (3).

// assumed: this file is declared in `runtime.rs` as
// `#[cfg(test)] #[path = "runtime_closed_tests.rs"] mod closed_tests;`.
// assumed: `Access::closed(&self) -> tokio::sync::watch::Receiver<Option<Closed>>`.

use std::sync::Arc;

use bytes::Bytes;

use crate::members::access::record::{encode, key_for, AclKind, AclValue, Via};
use crate::members::access::runtime::Access;
use crate::members::access::tally::Closed;
use crate::members::access::view::AccessView;
use crate::members::membership::Members;
use crate::transport::net::contract::{pid, secret};
use crate::transport::net::mem::{MemReplica, MemSync, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_DOMAINS, REPLICA_NET};
use crate::transport::net::{PeerId, ReplicaEntry};

mod managed_network {
    use super::*;

    #[tokio::test]
    async fn is_open_before_any_close_record() {
        // given: a managed network with a member and no close record
        let world = setup();
        // when
        world.refresh().await;
        // then
        assert_eq!(world.closed_now(), None);
    }

    #[tokio::test]
    async fn an_administrator_close_is_published() {
        // given: the root wrote a close record at second 5
        let world = setup();
        world.given_close(5, root());
        // when
        world.refresh().await;
        // then: the channel says who closed and when
        assert_eq!(
            world.closed_now(),
            Some(Closed {
                by: root(),
                at_unix: 5
            })
        );
    }

    #[tokio::test]
    async fn the_change_wakes_a_watcher_that_was_waiting() {
        // given: a receiver taken while the network is open
        let world = setup();
        world.refresh().await;
        let mut rx = world.access.closed();
        rx.mark_unchanged();
        // when: the close arrives
        world.given_close(5, root());
        world.refresh().await;
        // then: the receiver sees a change
        assert!(rx.has_changed().unwrap(), "the close must wake the watcher");
    }

    #[tokio::test]
    async fn a_refresh_without_news_does_not_wake_the_watcher() {
        // given: the network is closed and the watcher has seen it
        let world = setup();
        world.given_close(5, root());
        world.refresh().await;
        let mut rx = world.access.closed();
        rx.mark_unchanged();
        // when: the list is recomputed again with the same records
        world.refresh().await;
        // then
        assert!(!rx.has_changed().unwrap(), "the same value must not wake");
    }

    #[tokio::test]
    async fn a_close_of_a_non_administrator_stays_none() {
        // given: the plain member x wrote a close record
        let world = setup();
        world.given_close(5, member());
        // when
        world.refresh().await;
        // then
        assert_eq!(world.closed_now(), None);
    }
}

// --- setup and helpers ---

fn root() -> PeerId {
    pid(2)
}

fn member() -> PeerId {
    pid(3)
}

struct World {
    access: Access,
    shared: Arc<MemReplica>,
    _keep: (Arc<MemSyncNet>, Arc<MemSync>),
}

/// The root admitted me and the member `x`.
fn setup() -> World {
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
    let access = Access::from_parts(node.peer_id(), shared.clone(), own, members, root());
    let world = World {
        access,
        shared,
        _keep: (net, backend),
    };
    world.given_record(1, AclKind::Admit, root(), node.peer_id());
    world.given_record(1, AclKind::Admit, root(), member());
    world
}

impl World {
    /// A close record of `author` written at second `at`.
    fn given_close(&self, at: u64, author: PeerId) {
        self.given_record(at, AclKind::Close, author, author);
    }

    fn given_record(&self, at: u64, kind: AclKind, author: PeerId, subject: PeerId) {
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

    async fn refresh(&self) {
        self.access
            .refresh()
            .await
            .expect("the recompute must work on a healthy replica");
    }

    fn closed_now(&self) -> Option<Closed> {
        self.access.closed().borrow().clone()
    }
}
