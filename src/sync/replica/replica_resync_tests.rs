//! Periodic reconciliation of a replica with its topic neighbors: it brings an entry the live broadcast did not deliver,
//! and only the node with the smaller `PeerId` starts it.

use super::super::entry::seal;
use super::super::ids::net_id;
use super::super::replica_sync::resync_delay;
use super::super::test_support::{introduce, mesh_node, MeshNode};
use crate::net::contract::replica_cases::{is_remote, next_event};
use crate::net::contract::{must_finish, pid, secret, SEED_A, SEED_B};
use crate::net::mem::MemNet;
use crate::net::replica::{Replica, ReplicaEvent};
use crate::net::sync_backend::SyncBackend;
use crate::sync::flood::test_support::NETWORK;
use std::sync::Arc;
use std::time::Duration;

async fn net_replica(node: &MeshNode) -> Arc<dyn Replica> {
    must_finish(node.sync.open_replica("net")).await.expect("the net replica")
}

/// Two nodes that are topic neighbors after one explicit reconciliation; returns `(low, high)` by `PeerId`.
async fn neighbors(net: &Arc<MemNet>) -> (MeshNode, MeshNode) {
    let (a, b) = (mesh_node(net, SEED_A, NETWORK, None).await, mesh_node(net, SEED_B, NETWORK, None).await);
    introduce(&a, &b);
    let (low, high) = match pid(SEED_A).as_bytes() < pid(SEED_B).as_bytes() {
        true => (a, b),
        false => (b, a),
    };
    let (rl, rh) = (net_replica(&low).await, net_replica(&high).await);
    must_finish(rh.start_sync()).await.unwrap();
    let mut events = rl.subscribe();
    must_finish(rl.sync_with(pid_of(&high))).await.unwrap();
    let up = |e: &ReplicaEvent| matches!(e, ReplicaEvent::Neighbor { up: true, .. });
    next_event(&mut events, "Neighbor up", up).await;
    tokio::time::sleep(Duration::from_secs(1)).await;
    (low, high)
}

fn pid_of(node: &MeshNode) -> crate::net::id::PeerId {
    node.sync.host.open("net").unwrap().me()
}

/// Merges an entry of `author` into `node`'s replica without a live broadcast.
fn quiet_entry(node: &MeshNode, author: u8, key: &[u8]) {
    let w = seal(&secret(author), &net_id(&NETWORK), key, b"v", 5);
    node.sync.host.open("net").unwrap().lock().merge(w);
}

#[test]
fn delay_is_every_period_plus_up_to_a_quarter() {
    assert_eq!(resync_delay(0), Duration::from_secs(120));
    assert_eq!(resync_delay(u32::MAX), Duration::from_secs(150));
}

#[tokio::test(start_paused = true)]
async fn timer_brings_entry_the_broadcast_missed() {
    // given: neighbors; the larger node holds an entry nobody broadcast → when: the period passes → then: the smaller one has it
    let net = MemNet::new();
    let (low, high) = neighbors(&net).await;
    let author = if pid(SEED_A) == pid_of(&high) { SEED_A } else { SEED_B };
    quiet_entry(&high, author, b"k/");
    let rl = net_replica(&low).await;
    assert!(must_finish(rl.scan(b"k/")).await.unwrap().is_empty());
    let mut events = rl.subscribe();
    tokio::time::sleep(Duration::from_secs(160)).await;
    next_event(&mut events, "RemoteChange", is_remote).await;
    assert_eq!(must_finish(rl.scan(b"k/")).await.unwrap().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn only_smaller_peer_starts_reconciliation() {
    // given: neighbors → when: the larger node runs its periodic step → then: it starts no session, the smaller one does
    let net = MemNet::new();
    let (low, high) = neighbors(&net).await;
    let (rl, rh) = (low.sync.host.open("net").unwrap(), high.sync.host.open("net").unwrap());
    rh.resync_smaller();
    assert!(rh.claim(rl.me()).is_some(), "the larger node started a session");
    rl.resync_smaller();
    assert!(rl.claim(rh.me()).is_none(), "the smaller node started no session");
}
