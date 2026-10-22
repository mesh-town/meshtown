//! Tests of mem sync outside the contract suites (those are in `contract_tests`): reconciliation bounds, `put_raw` merge,
//! `replica_mem`, joining a topic later than a neighbor, selection by name from the registry.

use crate::transport::net::contract::replica_cases::{
    assert_no_change, expect_ok, get, is_remote, next_event, put, scan, val, Rep, K1, K2, QUIET,
};
use crate::transport::net::contract::{must_finish, pid, secret, SEED_A, SEED_B, STEP};
use crate::transport::net::id::PeerId;
use crate::transport::net::mem::{sync_ctor, MemReplica, MemSync, MemSyncNet};
use crate::transport::net::registry::{NetChoice, Registry};
use crate::transport::net::replica::{Replica, ReplicaEntry, ReplicaEvent};
use crate::transport::net::sync::SwarmEvent;
use crate::transport::net::sync_backend::{
    Swarm, SyncBackend, SyncCtx, REPLICA_DOMAINS, REPLICA_NET,
};
use std::sync::Arc;
use tokio::sync::broadcast;

/// Network seeds of a node (`network_seed` = `[9; 32]`, as in the contract environment) and of a foreign network; the broadcast topic.
const NETWORK: u8 = 9;
const OTHER_NETWORK: u8 = 10;
const TOPIC: [u8; 32] = [3; 32];

mod sync_with {
    use super::*;

    #[tokio::test]
    async fn unstarted_peer_reports_err_event() {
        // given: A has reconciliation on and an entry; B has replica "net" open but start_sync was not called
        let (_net, a, b) = pair(NETWORK);
        let ra = started(&a).await;
        let rb: Rep = open_net(&b);
        put(&ra, K1, "a1").await;
        let mut events = ra.subscribe();
        // when: A reconciles with B
        expect_ok("sync_with(B)", must_finish(ra.sync_with(pid(SEED_B))).await);
        // then: the call is Ok, the result is Synced from B with Err, the entries of A did not appear at B
        expect_synced_err(&mut events, pid(SEED_B)).await;
        assert!(
            authors(&rb).await.is_empty(),
            "B accepted an entry without start_sync"
        );
    }

    #[tokio::test]
    async fn peer_without_replica_reports_err_event() {
        // given: A has reconciliation on; node B is in the network but did not open replica "net"
        let (_net, a, _b) = pair(NETWORK);
        let ra = started(&a).await;
        let mut events = ra.subscribe();
        // when: A reconciles with B
        expect_ok("sync_with(B)", must_finish(ra.sync_with(pid(SEED_B))).await);
        // then: the call is Ok, the result is Synced from B with Err
        expect_synced_err(&mut events, pid(SEED_B)).await;
    }

    #[tokio::test]
    async fn peer_in_other_network_reports_err_and_shares_nothing() {
        // given: A and B are in different networks, both have reconciliation on and an entry of their own
        let (_net, a, b) = pair(OTHER_NETWORK);
        let (ra, rb) = (started(&a).await, started(&b).await);
        put(&ra, K1, "a1").await;
        put(&rb, K2, "b2").await;
        let mut events = ra.subscribe();
        // when: A reconciles with B
        expect_ok("sync_with(B)", must_finish(ra.sync_with(pid(SEED_B))).await);
        // then: the result is Synced with Err, each has only its own entry
        expect_synced_err(&mut events, pid(SEED_B)).await;
        tokio::time::sleep(QUIET / 3).await;
        assert_eq!(authors(&ra).await, [pid(SEED_A)], "in replica A");
        assert_eq!(authors(&rb).await, [pid(SEED_B)], "in replica B");
    }
}

mod put_raw {
    use super::*;

    #[tokio::test]
    async fn older_after_newer_changes_nothing_and_emits_nothing() {
        // given: an entry K1 with time 2000 arrived from B
        let mut raw = with_foreign_entry("new", 2_000).await;
        // when: an entry of the same (author, key) pair arrives with the old time 1000
        raw.mem.put_raw(foreign("old", 1_000));
        // then: the new entry stayed and there is no change event
        assert_foreign(&raw.rep, "new", 2_000).await;
        assert_no_change(&mut raw.events).await;
    }

    #[tokio::test]
    async fn newer_after_older_replaces_and_emits_remote_change() {
        // given: an entry K1 with time 1000 arrived from B
        let mut raw = with_foreign_entry("old", 1_000).await;
        // when: an entry of the same pair arrives with time 2000
        raw.mem.put_raw(foreign("new", 2_000));
        // then: the entry is replaced, the subscriber got RemoteChange
        next_event(
            &mut raw.events,
            "RemoteChange from the new entry",
            is_remote,
        )
        .await;
        assert_foreign(&raw.rep, "new", 2_000).await;
    }

    #[tokio::test]
    async fn equal_time_keeps_first_and_emits_nothing() {
        // given: an entry K1 with time 1000 arrived from B
        let mut raw = with_foreign_entry("first", 1_000).await;
        // when: an entry of the same pair arrives with the same time and a different value
        raw.mem.put_raw(foreign("second", 1_000));
        // then: the previous one stays, there is no change event
        assert_foreign(&raw.rep, "first", 1_000).await;
        assert_no_change(&mut raw.events).await;
    }
}

mod replica_mem {
    use super::*;

    #[tokio::test]
    async fn is_the_replica_open_replica_opens() {
        // given: node A
        let (_net, a, _b) = pair(NETWORK);
        // when: replica "net" is taken through replica_mem and through open_replica
        let mem = open_net(&a);
        let opened = expect_ok(
            "open_replica",
            must_finish(a.open_replica(REPLICA_NET)).await,
        );
        // then: it is one object with one id, an entry written through one handle is visible through the other
        assert!(
            Arc::ptr_eq(&(mem.clone() as Rep), &opened),
            "different replica objects"
        );
        assert_eq!(mem.id(), opened.id(), "different ids of one replica");
        put(&opened, K1, "v1").await;
        let seen = get(&(mem as Rep), pid(SEED_A), K1).await;
        assert!(
            seen.is_some(),
            "the entry is not visible through the second handle"
        );
    }

    #[tokio::test]
    async fn net_and_domains_ids_differ() {
        // given: node A; when: we take replicas "net" and "domains"
        let (_net, a, _b) = pair(NETWORK);
        let shared = open_net(&a);
        let own = a.replica_mem(REPLICA_DOMAINS).expect("domains");
        // then: the ids differ
        assert_ne!(shared.id(), own.id(), "two replicas with one id");
    }
}

mod swarm {
    use super::*;

    #[tokio::test]
    async fn peer_joining_later_links_both_ways() {
        // given: B joined the topic inviting A while A is not yet in the topic
        let (_net, a, b) = pair(NETWORK);
        let mut swarm_b = join(&b, &[pid(SEED_A)]).await;
        // when: A joins the same topic with no initial neighbors
        let mut swarm_a = join(&a, &[]).await;
        // then: both see NeighborUp of each other
        next_up(&mut swarm_a.events, pid(SEED_B)).await;
        next_up(&mut swarm_b.events, pid(SEED_A)).await;
    }

    #[tokio::test]
    async fn repeated_join_does_not_repeat_neighbor_up() {
        // given: A and B are neighbors, both have already received NeighborUp
        let (_net, a, b) = pair(NETWORK);
        let mut swarm_a = join(&a, &[]).await;
        let mut swarm_b = join(&b, &[pid(SEED_A)]).await;
        next_up(&mut swarm_a.events, pid(SEED_B)).await;
        next_up(&mut swarm_b.events, pid(SEED_A)).await;
        // when: B invites A again
        let again = must_finish(swarm_b.membership.join(&[pid(SEED_A)])).await;
        expect_ok("repeated join", again);
        // then: there is no second NeighborUp at either A or B
        tokio::time::sleep(QUIET).await;
        let ups_a = pending_ups(&mut swarm_a.events, pid(SEED_B));
        let ups_b = pending_ups(&mut swarm_b.events, pid(SEED_A));
        assert_eq!((ups_a, ups_b), (0, 0), "repeated NeighborUp at A and at B");
    }
}

mod registry {
    use super::*;

    #[tokio::test]
    async fn opens_mem_sync_by_name() {
        // given: a registry with sync "mem" on a fresh MemSyncNet
        let mut registry = Registry::new();
        registry.add_sync("mem", sync_ctor(MemSyncNet::new()));
        let choice = NetChoice {
            sync: "mem".into(),
            ..NetChoice::default()
        };
        // when: we open sync by the choice "mem"
        let opened = must_finish(registry.open_sync(&choice, ctx(SEED_A, NETWORK))).await;
        // then: the sync name is "mem"
        assert_eq!(opened.expect("open_sync").name(), "mem");
    }
}

/// Two nodes: A in network `NETWORK`, B in network `network_b`; the network (the first element) is held until the end of the test.
fn pair(network_b: u8) -> (Arc<MemSyncNet>, Arc<MemSync>, Arc<MemSync>) {
    let net = MemSyncNet::new();
    let a = net.backend(ctx(SEED_A, NETWORK));
    let b = net.backend(ctx(SEED_B, network_b));
    (net, a, b)
}

/// Node parameters: key from seed `seed`, network `network`, in-memory storage.
fn ctx(seed: u8, network: u8) -> SyncCtx {
    SyncCtx {
        secret: secret(seed),
        network_seed: [network; 32],
        store_dir: None,
    }
}

fn open_net(node: &MemSync) -> Arc<MemReplica> {
    node.replica_mem(REPLICA_NET).expect("replica \"net\"")
}

/// Replica "net" of a node with reconciliation on.
async fn started(node: &MemSync) -> Rep {
    let replica: Rep = open_net(node);
    expect_ok("start_sync", must_finish(replica.start_sync()).await);
    replica
}

async fn join(node: &MemSync, bootstrap: &[PeerId]) -> Swarm {
    expect_ok(
        "join_swarm",
        must_finish(node.join_swarm(TOPIC, bootstrap)).await,
    )
}

struct Raw {
    mem: Arc<MemReplica>,
    rep: Rep,
    events: broadcast::Receiver<ReplicaEvent>,
}

/// Replica "net" of node A with a foreign entry K1 from B (`value`, `ts`); the subscription was created before it, `RemoteChange`
/// was read. `mem` is for `put_raw`, `rep` is for reading.
async fn with_foreign_entry(value: &str, ts: u64) -> Raw {
    let (_net, a, _b) = pair(NETWORK);
    let mem = open_net(&a);
    let rep: Rep = mem.clone();
    let mut events = rep.subscribe();
    mem.put_raw(foreign(value, ts));
    next_event(&mut events, "RemoteChange from the first entry", is_remote).await;
    Raw { mem, rep, events }
}

/// Entry K1 from author B with the given value and time.
fn foreign(value: &str, ts_micros: u64) -> ReplicaEntry {
    ReplicaEntry {
        author: pid(SEED_B),
        key: K1.to_vec(),
        value: val(value),
        ts_micros,
    }
}

async fn assert_foreign(rep: &Rep, value: &str, ts_micros: u64) {
    let got = get(rep, pid(SEED_B), K1).await.expect("no entry of B");
    assert_eq!((got.value, got.ts_micros), (val(value), ts_micros));
}

/// Authors of all entries of the replica (with tombstones) in `scan` order.
async fn authors(rep: &Rep) -> Vec<PeerId> {
    scan(rep, b"").await.iter().map(|e| e.author).collect()
}

/// Waits for `Synced` from `peer` and checks that the result is an error.
async fn expect_synced_err(events: &mut broadcast::Receiver<ReplicaEvent>, peer: PeerId) {
    let is_synced =
        |e: &ReplicaEvent| matches!(e, ReplicaEvent::Synced { peer: p, .. } if *p == peer);
    let synced = next_event(events, "Synced from the neighbor", is_synced).await;
    let ReplicaEvent::Synced { outcome, .. } = synced else {
        unreachable!()
    };
    assert!(
        outcome.is_err(),
        "expected Synced with Err, got {outcome:?}"
    );
}

/// Waits up to `STEP` for the `NeighborUp(peer)` event; skips the others; a closed or lagging channel is also a failure.
async fn next_up(events: &mut broadcast::Receiver<SwarmEvent>, peer: PeerId) {
    let found = tokio::time::timeout(STEP, async {
        while let Ok(event) = events.recv().await {
            if matches!(event, SwarmEvent::NeighborUp(p) if p == peer) {
                return true;
            }
        }
        false
    });
    assert!(
        found.await == Ok(true),
        "NeighborUp({peer}) did not arrive within {STEP:?}"
    );
}

/// How many `NeighborUp(peer)` are in the channel (without waiting).
fn pending_ups(events: &mut broadcast::Receiver<SwarmEvent>, peer: PeerId) -> usize {
    let mut count = 0;
    while let Ok(event) = events.try_recv() {
        count += usize::from(matches!(event, SwarmEvent::NeighborUp(p) if p == peer));
    }
    count
}
