//! Tests for `MeshSync`: name, services, an unknown replica.

use super::test_support::mesh_node;
use super::NAME;
use crate::net::contract::{must_finish, SEED_A};
use crate::net::mem::MemNet;
use crate::net::sync_backend::SyncBackend;
use crate::net::{NetError, REPLICA, SYNC};
use crate::sync::flood::test_support::NETWORK;

#[tokio::test]
async fn name_is_mesh() {
    // given: a mesh node → when: we ask the name → then: "mesh"
    let node = mesh_node(&MemNet::new(), SEED_A, NETWORK, None).await;
    assert_eq!(node.sync.name(), "mesh");
    assert_eq!(NAME, "mesh");
}

#[tokio::test]
async fn services_are_sync_and_replica() {
    // given: a mesh node → when: we ask for the services → then: SYNC and REPLICA; the single service() is SYNC
    let node = mesh_node(&MemNet::new(), SEED_A, NETWORK, None).await;
    let ids: Vec<_> = node.sync.services().into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, vec![SYNC, REPLICA]);
    assert_eq!(node.sync.service().map(|(id, _)| id), Some(SYNC));
}

#[tokio::test]
async fn unknown_replica_is_config() {
    // given: a mesh node → when: we open a replica with a foreign name → then: Config
    let node = mesh_node(&MemNet::new(), SEED_A, NETWORK, None).await;
    let res = must_finish(node.sync.open_replica("no-such-one")).await;
    assert!(matches!(res, Err(NetError::Config(_))), "expected Config");
}

#[tokio::test]
async fn domains_not_served() {
    // given: two nodes → when: a REPLICA dial with the topic "domains" → then: the handshake is rejected, the link is closed
    use crate::net::contract::{pid, SEED_B};
    use crate::sync::flood::admit::{admission_key, dial_handshake};
    use crate::sync::replica::ids::domains_id;
    use crate::sync::replica::test_support::introduce;
    let net = MemNet::new();
    let a = mesh_node(&net, SEED_A, NETWORK, None).await;
    let b = mesh_node(&net, SEED_B, NETWORK, None).await;
    introduce(&a, &b);
    let conn = must_finish(a.stack.connect(pid(SEED_B), REPLICA)).await.expect("link");
    let mut pair = must_finish(conn.open()).await.expect("stream");
    let key = admission_key(&NETWORK);
    let res = dial_handshake(&mut pair, &key, pid(SEED_A), pid(SEED_B), domains_id().as_bytes()).await;
    assert!(res.is_err(), "domains was accepted by the service");
    must_finish(conn.closed()).await;
}
