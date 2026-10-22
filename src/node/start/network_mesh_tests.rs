//! The `mesh` sync in the node: the `sync_registry` opens it by name, the stack is built with the `SYNC` service,
//! three `mem` nodes flood messages, two started `start_node()` on a shared `mem` network become topic peers.
//! Sync opens before the stack is built: `add_protocol` after `serve` gives `Config`.

use crate::net::contract::{pid, must_finish, SEED_A, SEED_B, SEED_C};
use crate::net::sync_backend::{Swarm, SyncCtx, REPLICA_NET};
use crate::net::{NetChoice, NetStack, NetStackBuilder, NodeSecret, PeerId, SwarmEvent, SyncBackend, Transport};
use crate::net::mem::MemNet;
use crate::network_key::NetworkKey;
use crate::node::{self, NodeConfig};
use crate::sync::flood::test_support::{is_down, is_received, next, quiet, received_count, wait_ups, TOPIC};
use bytes::Bytes;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

/// The test node: transport, the `mesh` sync from the node registry and a stack with its service.
struct Mesh {
    transport: Arc<dyn Transport>,
    net: Arc<MemNet>,
    sync: Arc<dyn SyncBackend>,
    stack: NetStack,
}

impl Mesh {
    fn id(&self) -> PeerId {
        self.transport.me()
    }
}

fn mesh_choice() -> NetChoice {
    NetChoice {
        sync: "mesh".to_string(),
        ..NetChoice::default()
    }
}

fn sync_ctx(seed: u8) -> SyncCtx {
    SyncCtx {
        secret: NodeSecret::from_bytes(&[seed; 32]),
        network_seed: [9; 32],
        store_dir: None,
    }
}

/// The `seed` node: `open_sync("mesh")` before the stack is built, then `service` → `build` → `attach_stack`.
async fn mesh_node(net: &Arc<MemNet>, seed: u8) -> Mesh {
    let transport: Arc<dyn Transport> = net.transport(&NodeSecret::from_bytes(&[seed; 32]));
    let registry = super::sync_registry();
    let sync = registry
        .open_sync(&mesh_choice(), sync_ctx(seed))
        .await
        .expect("mesh opens");
    let (id, handler) = sync.service().expect("mesh has a service");
    let stack = NetStackBuilder::new(NodeSecret::from_bytes(&[seed; 32]))
        .transport(transport.clone())
        .serve(id, handler)
        .build()
        .expect("stack built");
    sync.attach_stack(stack.downgrade());
    Mesh {
        transport,
        net: net.clone(),
        sync,
        stack,
    }
}

/// `who` knows the address of `whom`.
fn know(who: &Mesh, whom: &Mesh) {
    who.stack.add_hints(who.net.addr_of(whom.id()).expect("peer slot is alive"));
}

async fn enter(node: &Mesh, peers: &[PeerId]) -> Swarm {
    match must_finish(node.sync.join_swarm(TOPIC, peers)).await {
        Ok(swarm) => swarm,
        Err(e) => panic!("join_swarm did not join the topic: {e}"),
    }
}

/// A line A–B–C: B dials A, C dials B; A and C don't know each other.
async fn line() -> (Mesh, Mesh, Mesh, [Swarm; 3]) {
    let net = MemNet::new();
    let (a, b, c) = (mesh_node(&net, SEED_A).await, mesh_node(&net, SEED_B).await, mesh_node(&net, SEED_C).await);
    know(&b, &a);
    know(&c, &b);
    let mut sa = enter(&a, &[]).await;
    let mut sb = enter(&b, &[a.id()]).await;
    let mut sc = enter(&c, &[b.id()]).await;
    wait_ups(&mut sa.events, &[b.id()]).await;
    wait_ups(&mut sb.events, &[a.id(), c.id()]).await;
    wait_ups(&mut sc.events, &[b.id()]).await;
    (a, b, c, [sa, sb, sc])
}

#[tokio::test]
async fn registry_opens_mesh_with_own_replicas() {
    // given: the node registry on loopback
    let registry = super::sync_registry();
    // when: opening the "mesh" sync
    let sync = registry
        .open_sync(&mesh_choice(), sync_ctx(SEED_A))
        .await
        .expect("mesh opens");
    // then: the name is mesh, the "net" replica opens (its own, without iroh-docs), there are two services
    assert_eq!(sync.name(), "mesh");
    assert!(sync.open_replica(REPLICA_NET).await.is_ok(), "net replica");
    assert_eq!(sync.services().len(), 2, "SYNC and REPLICA");
}

#[tokio::test]
async fn line_forwards_on_mem() {
    // given: a line A–B–C on mem
    let (a, _b, c, [sa, sb, mut sc]) = line().await;
    // when: A sends a message
    must_finish(sa.membership.broadcast(Bytes::from("hello from a #1"))).await.expect("broadcast");
    // then: C received it from B with the same bytes
    let got = next(&mut sc.events, is_received).await;
    let SwarmEvent::Received { from, msg } = got else { panic!("not Received") };
    assert_eq!((from, msg), (pid(SEED_B), Bytes::from("hello from a #1")));
    drop((a, c, sb));
}

#[tokio::test]
async fn triangle_delivers_once_on_mem() {
    // given: a triangle A, B, C on mem
    let net = MemNet::new();
    let (a, b, c) = (mesh_node(&net, SEED_A).await, mesh_node(&net, SEED_B).await, mesh_node(&net, SEED_C).await);
    know(&b, &a);
    know(&c, &a);
    know(&c, &b);
    let sa = enter(&a, &[]).await;
    let mut sb = enter(&b, &[a.id()]).await;
    let mut sc = enter(&c, &[a.id(), b.id()]).await;
    wait_ups(&mut sb.events, &[a.id()]).await;
    wait_ups(&mut sc.events, &[a.id(), b.id()]).await;
    // when: A sends a message
    must_finish(sa.membership.broadcast(Bytes::from("hello from a #1"))).await.expect("broadcast");
    // then: C has exactly one Received
    next(&mut sc.events, is_received).await;
    assert_eq!(received_count(&quiet(&mut sc.events).await), 0, "C received it twice");
}

#[tokio::test]
async fn neighbor_down_when_stack_closes() {
    // given: a line A–B–C
    let (_a, b, _c, [mut sa, _sb, mut sc]) = line().await;
    // when: B's stack is closed
    b.stack.close().await;
    // then: A and C get NeighborDown(B)
    next(&mut sa.events, is_down(b.id())).await;
    next(&mut sc.events, is_down(b.id())).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn started_nodes_become_mesh_neighbors() {
    // given: two nodes of one network with net.sync = "mesh"
    let key = NetworkKey::generate().unwrap();
    let mut cfg = NodeConfig::for_loopback_test(key);
    cfg.settings.net_choice.sync = "mesh".to_string();
    let a = node::start_node(cfg.clone()).await.unwrap();
    let b = node::start_node(cfg).await.unwrap();
    // when: A learns B's address
    a.introduce(crate::net::PeerAddr::new(b.id)).await;
    // then: both are members and within 20 s each has one topic peer
    let limit = tokio::time::Instant::now() + Duration::from_secs(20);
    let ready = loop {
        let members = a.members.is_member(b.id) && b.members.is_member(a.id);
        let n = (a.gossip_neighbor_count.load(Ordering::Relaxed), b.gossip_neighbor_count.load(Ordering::Relaxed));
        if members && n == (1, 1) || tokio::time::Instant::now() >= limit {
            break (members, n);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    assert_eq!(ready, (true, (1, 1)), "members and topic peers within 20 s");
    a.shutdown().await;
    b.shutdown().await;
}

/// Waits until `done` becomes true, no longer than 20 s; returns the last value.
async fn within_20s<F, Fut>(mut done: F) -> bool
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let limit = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        let ok = done().await;
        if ok || tokio::time::Instant::now() >= limit {
            return ok;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn started_nodes_share_net_records_on_mesh() {
    // given: two nodes of one network with net.sync = "mesh"
    let key = NetworkKey::generate().unwrap();
    let mut cfg = NodeConfig::for_loopback_test(key);
    cfg.settings.net_choice.sync = "mesh".to_string();
    let a = node::start_node(cfg.clone()).await.unwrap();
    let b = node::start_node(cfg).await.unwrap();
    // when: A learns B's address
    a.introduce(crate::net::PeerAddr::new(b.id)).await;
    // then: within 20 s the `node/name/` record of node A is in node B's "net" replica (it arrives only through the REPLICA reconciliation)
    let net = b.domains.shared_replica();
    let seen = within_20s(|| async {
        let names = crate::node_name::read_node_names(net.as_ref()).await.unwrap_or_default();
        names.contains_key(&a.id)
    })
    .await;
    assert!(seen, "node A's name did not reach node B's replica within 20 s");
    a.shutdown().await;
    b.shutdown().await;
}
