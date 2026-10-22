//! Joining through a v2 invitation while the membership list is consulted (`open_list = false`):
//! the redeemer is admitted by the issuer's record, then both sides see each other as members,
//! while an outsider with the same network key stays out.
//!
//! What it does: three loopback nodes of one network, one invitation issued and redeemed with a
//! permanent key, then a node started with that key and the peers from the invitation.
//! What it does not do: no web layer, no relay; every wait has a ceiling so a hang fails the test.

use std::time::Duration;

use meshtown::net::NodeSecret;
use meshtown::network_key::NetworkKey;
use meshtown::node::{self, NodeConfig, RunningNode};

#[path = "common/invite_nodes.rs"]
mod common;
#[path = "common/invite_mesh.rs"]
mod mesh;
use common::{quick_opts, redeem_via, wait_until, ENTRY_LIMIT_SECS, OWNER_SEED};
use mesh::deal;

/// The ceiling of one wait for a membership change.
const LIMIT: Duration = Duration::from_secs(20);
/// How long the outsider is watched after the newcomer has joined.
const WATCH: Duration = Duration::from_secs(3);

/// A node that consults the membership list, with no SOCKS listener.
async fn start_listed(key: &NetworkKey, owner: Option<NodeSecret>, peers: Vec<meshtown::net::PeerAddr>) -> RunningNode {
    let mut cfg = NodeConfig::for_loopback_test(key.clone());
    cfg.open_list = false;
    cfg.socks.enabled = false;
    cfg.identity_secret = owner;
    cfg.initial_peers = peers;
    node::start_node(cfg).await.unwrap()
}

async fn wait_member(node: &RunningNode, who: meshtown::net::PeerId, what: &str) {
    let seen = || node.members.is_member(who);
    assert!(wait_until(LIMIT, seen).await, "{what}: not a member within {LIMIT:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn invite_v2_admits_the_newcomer_while_the_list_is_on() {
    // given: node A with the list on, an invitation issued by it, and an outsider F with the same key
    let key = NetworkKey::generate().unwrap();
    let a = start_listed(&key, None, Vec::new()).await;
    let (_dealt, parsed) = deal(&a).await;
    let outsider = start_listed(&key, None, parsed.peers.clone()).await;
    let owner = NodeSecret::from_bytes(&[OWNER_SEED; 32]);
    let owner_id = owner.peer_id();

    // when: the invitation is redeemed with the permanent key
    let got = redeem_via(&parsed.head, &parsed.peers, quick_opts(&parsed.head, ENTRY_LIMIT_SECS)).await;
    let granted = got.expect("the invitation must be redeemed while the list is on");
    assert_eq!(granted.subkeys().network_id, key.subkeys().network_id, "the granted key must be the network's");

    // then: the issuer lets the redeemer in by the invitation record
    let allowed = || a.members.access().allows(owner_id);
    assert!(wait_until(LIMIT, allowed).await, "the issuer must admit the redeemer within {LIMIT:?}");

    // when: the redeemer starts as a node with the granted key and the peers of the invitation
    let newcomer = start_listed(&granted, Some(owner), parsed.peers.clone()).await;
    assert_eq!(newcomer.id, owner_id, "the node must run under the redeemer's permanent key");

    // then: both sides see each other as members
    wait_member(&newcomer, a.id, "newcomer sees issuer").await;
    wait_member(&a, newcomer.id, "issuer sees newcomer").await;

    // control: the outsider, started before the newcomer joined, is still not let in
    wait_until(WATCH, || a.members.is_member(outsider.id)).await;
    assert!(!a.members.is_member(outsider.id), "an outsider must not become a member");
    assert!(!a.members.access().allows(outsider.id), "an outsider must not be allowed by the list");

    newcomer.shutdown().await;
    outsider.shutdown().await;
    a.shutdown().await;
}
