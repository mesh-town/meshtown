//! Tests for the replica over the `mem` network: saving what was received, broadcast, peers, a repeated `sync_with`, a corrupt file.

use super::entry::{seal, WireEntry};
use super::file;
use super::ids::net_id;
use super::test_support::{introduce, mesh_node, temp_dir, MeshNode};
use super::wire::encode_push;
use crate::net::contract::{pid, replica_cases::{assert_no_change, is_remote, next_event, QUIET}};
use crate::net::contract::{secret, must_finish, SEED_A, SEED_B, STEP};
use crate::net::error::NetError;
use crate::net::mem::MemNet;
use crate::net::replica::{Replica, ReplicaEvent};
use crate::net::sync_backend::SyncBackend;
use crate::sync::flood::test_support::NETWORK;
use bytes::Bytes;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

async fn net_replica(node: &MeshNode) -> Arc<dyn Replica> {
    must_finish(node.sync.open_replica("net")).await.expect("the net replica")
}

fn store(base: &Path, who: &str) -> PathBuf {
    base.join(who).join("docs-store")
}

fn net_file(base: &Path, who: &str) -> PathBuf {
    file::path_for(&base.join(who).join("replica"), "net", &net_id(&NETWORK))
}

fn is_synced(e: &ReplicaEvent) -> bool {
    matches!(e, ReplicaEvent::Synced { .. })
}

fn val(s: &str) -> Bytes {
    Bytes::copy_from_slice(s.as_bytes())
}

/// Waits up to `limit` until the `"net"` file holds `n` entries.
async fn wait_file(path: &Path, n: usize, limit: Duration) {
    let polling = async {
        loop {
            let got = std::fs::read(path).ok().and_then(|b| file::decode(&net_id(&NETWORK), &b).ok());
            if got.is_some_and(|(_, e)| e.len() == n) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    };
    tokio::time::timeout(limit, polling).await.expect("the net file was not completed");
}

/// Two network nodes, both with `"net"` reconciliation enabled; `low` is the node with the smaller `PeerId`.
async fn started(net: &Arc<MemNet>) -> (MeshNode, MeshNode) {
    let (a, b) = (mesh_node(net, SEED_A, NETWORK, None).await, mesh_node(net, SEED_B, NETWORK, None).await);
    introduce(&a, &b);
    must_finish(net_replica(&a).await.start_sync()).await.expect("start A");
    must_finish(net_replica(&b).await.start_sync()).await.expect("start B");
    match pid(SEED_A).as_bytes() < pid(SEED_B).as_bytes() {
        true => (a, b),
        false => (b, a),
    }
}

mod persist {
    use super::*;

    #[tokio::test]
    async fn remote_merge_saved_later() {
        // given: B has a directory, A has an entry → when: A reconciled with B → then: within 1 s + STEP A's entry is in B's file
        let (net, base) = (MemNet::new(), temp_dir("remote"));
        let a = mesh_node(&net, SEED_A, NETWORK, None).await;
        let b = mesh_node(&net, SEED_B, NETWORK, Some(store(&base, "b"))).await;
        introduce(&a, &b);
        let (ra, rb) = (net_replica(&a).await, net_replica(&b).await);
        must_finish(rb.start_sync()).await.unwrap();
        must_finish(ra.set_own(b"k/", val("v"))).await.unwrap();
        must_finish(ra.sync_with(pid(SEED_B))).await.unwrap();
        wait_file(&net_file(&base, "b"), 1, Duration::from_secs(1) + STEP).await;
    }

    #[tokio::test]
    async fn partial_merge_saved_when_session_fails() {
        // given: a session merged the peer's entry and broke off with an error → then: RemoteChange and the entry in the file within 1 s + STEP
        let (net, base) = (MemNet::new(), temp_dir("partial"));
        let b = mesh_node(&net, SEED_B, NETWORK, Some(store(&base, "b"))).await;
        let replica = b.sync.host.open("net").unwrap();
        let mut events = replica.subscribe();
        let before = replica.lock().generation();
        replica.lock().merge(seal(&secret(SEED_A), &net_id(&NETWORK), b"k/", b"v", 5));
        replica.finish(pid(SEED_A), before, Err(NetError::Timeout));
        next_event(&mut events, "RemoteChange", is_remote).await;
        wait_file(&net_file(&base, "b"), 1, Duration::from_secs(1) + STEP).await;
    }
}

mod push {
    use super::*;

    #[tokio::test]
    async fn bad_signature_ignored() {
        // given: A and B reconciled → when: a Push with a corrupted signature goes to the broadcast topic → then: B has no RemoteChange
        let net = MemNet::new();
        let (a, b) = started(&net).await;
        let (ra, rb) = (net_replica(&a).await, net_replica(&b).await);
        let mut synced = ra.subscribe();
        must_finish(ra.sync_with(b.sync.host.open("net").unwrap().me())).await.unwrap();
        next_event(&mut synced, "reconciliation result", is_synced).await;
        let mut events = rb.subscribe();
        let mut bad: WireEntry = seal(&secret(SEED_A), &net_id(&NETWORK), b"k/", b"v", 5);
        bad.sig[0][0] ^= 1;
        let body = encode_push(&net_id(&NETWORK), vec![bad]).unwrap();
        let swarm = a.sync.host.open("net").unwrap().net.topic_membership.get().cloned().expect("the topic is up");
        must_finish(swarm.broadcast(body)).await.unwrap();
        assert_no_change(&mut events).await;
        assert!(must_finish(rb.scan(b"")).await.unwrap().is_empty(), "a corrupt entry got into the map");
    }
}

mod neighbor {
    use super::*;

    #[tokio::test]
    async fn events_follow_topic() {
        // given: two replicas with reconciliation → when: sync_with, then a network break → then: Neighbor up, then Neighbor down
        let net = MemNet::new();
        let (a, b) = started(&net).await;
        let ra = net_replica(&a).await;
        let mut events = ra.subscribe();
        let peer = b.sync.host.open("net").unwrap().me();
        must_finish(ra.sync_with(peer)).await.unwrap();
        let up = |e: &ReplicaEvent| matches!(e, ReplicaEvent::Neighbor { peer: p, up: true } if *p == peer);
        next_event(&mut events, "Neighbor up", up).await;
        net.cut(a.sync.host.open("net").unwrap().me(), peer);
        let down = |e: &ReplicaEvent| matches!(e, ReplicaEvent::Neighbor { peer: p, up: false } if *p == peer);
        next_event(&mut events, "Neighbor down", down).await;
    }
}

mod sync_with {
    use super::*;

    #[tokio::test]
    async fn repeat_during_session_is_ok() {
        // given: the node with the smaller PeerId dials → when: two sync_with in a row → then: one Synced within QUIET after the first
        let net = MemNet::new();
        let (low, high) = started(&net).await;
        let peer = high.sync.host.open("net").unwrap().me();
        let r = net_replica(&low).await;
        let mut events = r.subscribe();
        let (first, second) = tokio::join!(must_finish(r.sync_with(peer)), must_finish(r.sync_with(peer)));
        first.unwrap();
        second.unwrap();
        next_event(&mut events, "Synced", is_synced).await;
        let more = tokio::time::timeout(QUIET, next_event(&mut events, "second Synced", is_synced)).await;
        assert!(more.is_err(), "extra session: {more:?}");
    }

    #[tokio::test]
    async fn starts_sync_itself() {
        // given: A has no start_sync → when: sync_with → then: Synced arrives and the entry arrived
        let net = MemNet::new();
        let (a, b) = (mesh_node(&net, SEED_A, NETWORK, None).await, mesh_node(&net, SEED_B, NETWORK, None).await);
        introduce(&a, &b);
        let (ra, rb) = (net_replica(&a).await, net_replica(&b).await);
        must_finish(rb.start_sync()).await.unwrap();
        must_finish(ra.set_own(b"k/", val("v"))).await.unwrap();
        let mut events = ra.subscribe();
        must_finish(ra.sync_with(pid(SEED_B))).await.unwrap();
        next_event(&mut events, "Synced", is_synced).await;
        assert_eq!(must_finish(rb.scan(b"")).await.unwrap().len(), 1);
        let mut at_b = rb.subscribe();
        must_finish(ra.set_own(b"k2/", val("w"))).await.unwrap();
        next_event(&mut at_b, "broadcast arrived", is_remote).await;
    }
}

mod corrupt {
    use super::*;

    #[tokio::test]
    async fn net_broken_starts_empty_and_restores() {
        // given: A's own entries reached B, A's file is corrupt, there is no .bak
        let (net1, base) = (MemNet::new(), temp_dir("broken-net"));
        let a = mesh_node(&net1, SEED_A, NETWORK, Some(store(&base, "a"))).await;
        let b = mesh_node(&net1, SEED_B, NETWORK, Some(store(&base, "b"))).await;
        introduce(&a, &b);
        let (ra, rb) = (net_replica(&a).await, net_replica(&b).await);
        must_finish(rb.start_sync()).await.unwrap();
        must_finish(ra.set_own(b"k/", val("v"))).await.unwrap();
        must_finish(ra.sync_with(pid(SEED_B))).await.unwrap();
        wait_file(&net_file(&base, "b"), 1, Duration::from_secs(1) + STEP).await;
        wait_file(&net_file(&base, "a"), 1, Duration::from_secs(1) + STEP).await;
        std::fs::write(net_file(&base, "a"), b"garbage").unwrap();
        let _ = std::fs::remove_file(format!("{}.bak", net_file(&base, "a").display()));
        // when: both nodes restart on a new network → then: A starts empty, .broken-* next to it, after reconciliation the entry came back
        let net2 = MemNet::new();
        let a = mesh_node(&net2, SEED_A, NETWORK, Some(store(&base, "a"))).await;
        let b = mesh_node(&net2, SEED_B, NETWORK, Some(store(&base, "b"))).await;
        introduce(&a, &b);
        let (ra, rb) = (net_replica(&a).await, net_replica(&b).await);
        assert!(must_finish(ra.scan(b"")).await.unwrap().is_empty(), "A did not start empty");
        let names = std::fs::read_dir(base.join("a").join("replica")).unwrap();
        assert!(names.flatten().any(|e| e.file_name().to_string_lossy().contains(".broken-")), "no .broken-*");
        must_finish(rb.start_sync()).await.unwrap();
        must_finish(ra.sync_with(pid(SEED_B))).await.unwrap();
        let polling = async {
            while must_finish(ra.scan(b"")).await.unwrap().is_empty() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        };
        tokio::time::timeout(STEP, polling).await.expect("the own entries did not return");
    }
}
