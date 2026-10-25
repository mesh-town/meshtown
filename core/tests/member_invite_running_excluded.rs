//! A node excluded from a network that keeps running does not hand out the network key to a computer
//! redeeming a new invite; the first node refuses it for good. This test lives in its own binary
//! because it shares the owner key and the process network with the other excluded-node test. The two tests
//! here take turns: both use the same owner key.

use meshtown_core::invite::redeem::RedeemError;
use meshtown_core::members::access::actions::{apply, MemberAction};
use meshtown_core::members::network_key::NetworkKey;
use meshtown_core::node::{self, NodeConfig, RunningNode};
use meshtown_core::transport::net::{NodeSecret, PeerAddr};
use std::path::PathBuf;

#[path = "common/invite_nodes.rs"]
mod common;
#[path = "common/invite_mesh.rs"]
mod mesh;
use common::{addr_of, quick_opts, redeem_via, wait_until, ENTRY_LIMIT_SECS, OWNER_SEED};
use mesh::deal;

/// The ceiling of one wait for a membership change.
const LIMIT: std::time::Duration = std::time::Duration::from_secs(20);

/// Held by a test for its whole run: the tests share the owner key and the process network.
static TURN: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_running_excluded_node_does_not_hand_out_a_key() {
    let _turn = TURN.lock().await;
    // given: a network of two, the second joined by an invite and then excluded by the first
    let (a, excluded, _cfg) = given_excluded_node(None).await;
    // when: the excluded node keeps running, listed first among the addresses of a new invite,
    // and its computer redeems the invite under its own key
    // then: the running excluded node gives no key; the first node refuses for good
    then_redeem_is_refused(&a, &excluded).await;
    excluded.shutdown().await;
    a.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_restarted_excluded_node_does_not_hand_out_a_key() {
    let _turn = TURN.lock().await;
    // given: a network of two, the second one excluded by the first, restarted with the same directory
    let dir = scratch_store_dir();
    let (a, excluded, cfg) = given_excluded_node(Some(dir)).await;
    excluded.shutdown().await;
    let restarted = node::start_node(cfg).await.unwrap();
    // when / then: the restarted node gives no key either; the first node refuses for good
    then_redeem_is_refused(&a, &restarted).await;
    restarted.shutdown().await;
    a.shutdown().await;
}

// ---- helpers ----

/// A network of two: the second node joined by an invite and was excluded by the first, and both know it.
/// With a `store_dir` the second node keeps its lists on disk; its config comes back for a restart.
async fn given_excluded_node(store_dir: Option<PathBuf>) -> (RunningNode, RunningNode, NodeConfig) {
    let key = NetworkKey::generate().unwrap();
    let a = start_listed(&key, None, Vec::new()).await;
    let (_dealt, first) = deal(&a).await;
    let owner = NodeSecret::from_bytes(&[OWNER_SEED; 32]);
    let granted = redeem_via(
        &first.head,
        &first.peers,
        quick_opts(&first.head, ENTRY_LIMIT_SECS),
    )
    .await
    .expect("the first invite must be redeemed");
    let mut cfg = listed_config(&granted, Some(owner.clone()), first.peers.clone());
    cfg.docs_store_dir = store_dir;
    let excluded = node::start_node(cfg.clone()).await.unwrap();
    let both = || a.members.is_member(excluded.id) && excluded.members.is_member(a.id);
    assert!(
        wait_until(LIMIT, both).await,
        "the nodes did not become members within {LIMIT:?}"
    );
    let voted = apply(&a.access, MemberAction::Exclude(excluded.id, None)).await;
    assert_eq!(voted, Ok(true), "one vote must exclude the node");
    let informed = || !a.members.access().allows(owner.peer_id()) && knows_own_exclusion(&excluded);
    assert!(
        wait_until(LIMIT, informed).await,
        "the exclusion did not reach both nodes"
    );
    (a, excluded, cfg)
}

/// Whether the node's own list says that the node itself is excluded.
fn knows_own_exclusion(node: &RunningNode) -> bool {
    let verdicts = node.members.access().verdicts();
    verdicts.is_some_and(|v| v.ledger.excluded.contains_key(&node.id))
}

/// A new invite of `a` lists `excluded` first; redeeming it fails for good because the node gives no key.
async fn then_redeem_is_refused(a: &RunningNode, excluded: &RunningNode) {
    let (_dealt, mut second) = deal(a).await;
    second.peers = vec![addr_of(excluded), addr_of(a)];
    let got = redeem_via(
        &second.head,
        &second.peers,
        quick_opts(&second.head, ENTRY_LIMIT_SECS),
    )
    .await;
    let error = got.expect_err("an excluded node must not hand out the key");
    assert_eq!(error, RedeemError::Excluded);
}

/// A store directory of its own in the temporary directory (only its parent is created).
fn scratch_store_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
        "mesh-excluded-restart-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("test temporary directory");
    root.join("docs-store")
}

/// A node that consults the membership list, with no SOCKS listener.
async fn start_listed(
    key: &NetworkKey,
    owner: Option<NodeSecret>,
    peers: Vec<PeerAddr>,
) -> RunningNode {
    node::start_node(listed_config(key, owner, peers))
        .await
        .unwrap()
}

fn listed_config(key: &NetworkKey, owner: Option<NodeSecret>, peers: Vec<PeerAddr>) -> NodeConfig {
    let mut cfg = NodeConfig::for_loopback_test(key.clone());
    cfg.open_list = false;
    cfg.identity_secret = owner;
    cfg.initial_peers = peers;
    cfg
}
