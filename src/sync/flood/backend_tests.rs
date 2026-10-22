//! Tests for the flood sync: name, service, replica delegate, one topic per node, the stack's weak handle.

use super::test_support::{fast, mem_node, NETWORK};
use super::{sync_ctor, NAME};
use crate::net::contract::{pid, secret, must_finish, SEED_A};
use crate::net::mem::{sync_ctor as mem_ctor, MemNet, MemSyncNet};
use crate::net::sync_backend::{SyncBackend, SyncCtx, REPLICA_NET};
use crate::net::{NetError, SYNC};

fn ctx() -> SyncCtx {
    SyncCtx {
        secret: secret(SEED_A),
        network_seed: NETWORK,
        store_dir: None,
    }
}

#[tokio::test]
async fn name_is_mesh() {
    // given: a flood node → when: we ask the name → then: "mesh"
    let node = mem_node(&MemNet::new(), &MemSyncNet::new(), SEED_A, NETWORK, fast());
    assert_eq!(node.sync.name(), "mesh");
    assert_eq!(NAME, "mesh");
}

#[tokio::test]
async fn service_is_sync() {
    // given: a flood node → when: we ask for the service for the stack builder → then: SYNC
    let node = mem_node(&MemNet::new(), &MemSyncNet::new(), SEED_A, NETWORK, fast());
    let (id, _handler) = node.sync.service().expect("there is a service");
    assert_eq!(id, SYNC);
}

#[tokio::test]
async fn open_replica_delegates() {
    // given: a MemSync delegate → when: we open "net" and an unknown name → then: "net" is Ok, the error is the same as the delegate's
    let hub = MemSyncNet::new();
    let node = mem_node(&MemNet::new(), &hub, SEED_A, NETWORK, fast());
    assert!(must_finish(node.sync.open_replica(REPLICA_NET)).await.is_ok());
    let own = must_finish(node.sync.open_replica("no-such-one")).await.err();
    let theirs = must_finish(hub.backend(ctx()).open_replica("no-such-one")).await.err();
    assert_eq!(own.map(|e| e.to_string()), theirs.map(|e| e.to_string()));
}

#[tokio::test]
async fn second_join_of_topic_is_error() {
    // given: a topic is already open → when: we enter it a second time → then: Config
    let node = mem_node(&MemNet::new(), &MemSyncNet::new(), SEED_A, NETWORK, fast());
    let first = must_finish(node.sync.join_swarm([5; 32], &[])).await;
    let second = must_finish(node.sync.join_swarm([5; 32], &[pid(2)])).await;
    assert!(first.is_ok());
    assert!(matches!(second, Err(NetError::Config(_))));
}

#[tokio::test]
async fn attached_sync_does_not_hold_stack() {
    // given: the sync is attached to the stack → when: the only NetStack is dropped → then: upgrade gives None
    let node = mem_node(&MemNet::new(), &MemSyncNet::new(), SEED_A, NETWORK, fast());
    let weak = node.stack.downgrade();
    assert!(weak.upgrade().is_some());
    let keep_sync = node.sync.clone();
    drop(node);
    assert!(weak.upgrade().is_none());
    drop(keep_sync);
}

#[tokio::test]
async fn sync_ctor_wraps_inner() {
    // given: a constructor over mem → when: we create the sync → then: the name is mesh, the "net" replica opens
    let wrapped = sync_ctor(mem_ctor(MemSyncNet::new()));
    let sync = must_finish(wrapped(ctx())).await.expect("the sync was created");
    assert_eq!(sync.name(), "mesh");
    assert!(must_finish(sync.open_replica(REPLICA_NET)).await.is_ok());
}
