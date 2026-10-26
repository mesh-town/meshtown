//! A node excluded from a network that keeps running does not hand out the network key to a computer
//! redeeming a new invite; the first node refuses it for good. This test lives in its own binary
//! because it shares the owner key and the process network with the other excluded-node test. The two tests
//! here take turns: both use the same owner key.

use meshtown_core::invite::redeem::RedeemError;
use meshtown_core::members::access::actions::{apply, MemberAction};
use meshtown_core::node::{self, NodeConfig, RunningNode};
use std::path::PathBuf;

#[path = "common/invite_nodes.rs"]
mod common;
#[path = "common/managed_net.rs"]
mod managed;
#[path = "common/invite_mesh.rs"]
mod mesh;
use common::{
    addr_of, quick_opts, redeem_via, short_opts, wait_until, ENTRY_LIMIT_SECS, OWNER_SEED,
};
use managed::{wait_can_invite, ManagedNet};
use mesh::deal;

/// The ceiling of one wait for a membership change.
const LIMIT: std::time::Duration = std::time::Duration::from_secs(20);

/// Held by a test for its whole run: the tests share the owner key and the process network.
static TURN: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_running_excluded_node_does_not_hand_out_a_key() {
    let _turn = TURN.lock().await;
    // given: a network of two, the second joined by an invite and then excluded by the first
    let net = ManagedNet::new();
    let (a, excluded, _cfg) = given_excluded_node(&net, None).await;
    // when: the excluded node keeps running, the only address of a new invite, and its computer redeems it
    // then: the running excluded node gives no key
    then_redeem_is_refused(&a, &excluded).await;
    excluded.shutdown().await;
    a.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_restarted_excluded_node_does_not_hand_out_a_key() {
    let _turn = TURN.lock().await;
    // given: a network of two, the second one excluded by the first, restarted with the same directory
    let dir = scratch_store_dir();
    let net = ManagedNet::new();
    let (a, excluded, cfg) = given_excluded_node(&net, Some(dir)).await;
    excluded.shutdown().await;
    let restarted = node::start_node(cfg).await.unwrap();
    // when / then: the restarted node gives no key either
    then_redeem_is_refused(&a, &restarted).await;
    restarted.shutdown().await;
    a.shutdown().await;
}

// ---- helpers ----

/// A network of two: the second node joined by an invite and was excluded by the first, and both know it.
/// With a `store_dir` the second node keeps its lists on disk; its config comes back for a restart.
async fn given_excluded_node(
    net: &ManagedNet,
    store_dir: Option<PathBuf>,
) -> (RunningNode, RunningNode, NodeConfig) {
    let a = net.start_root(net.base.clone()).await;
    wait_can_invite(&a).await;
    let (_dealt, first) = deal(&a).await;
    let owner_id = net
        .member_config(OWNER_SEED)
        .identity_secret
        .unwrap()
        .peer_id();
    let granted = redeem_via(
        &first.head,
        &first.peers,
        quick_opts(&first.head, ENTRY_LIMIT_SECS),
    )
    .await
    .expect("the first invite must be redeemed");
    assert_eq!(
        first.head.reply_key(&granted).to_bytes(),
        net.key.to_bytes()
    );
    let mut cfg = net.member_config(OWNER_SEED);
    cfg.initial_peers = first.peers.clone();
    cfg.docs_store_dir = store_dir;
    let excluded = node::start_node(cfg.clone()).await.unwrap();
    let both = || a.members.is_member(excluded.id) && excluded.members.is_member(a.id);
    assert!(
        wait_until(LIMIT, both).await,
        "the nodes did not become members within {LIMIT:?}"
    );
    let excluded_now = apply(&a.access, MemberAction::Exclude(excluded.id, None)).await;
    assert_eq!(excluded_now, Ok(true), "the creator must exclude the node");
    let informed = || !a.members.access().allows(owner_id) && knows_own_exclusion(&excluded);
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

/// A new invite of `a` that names only `excluded`: the node is not let in through it and no key comes back;
/// retrying until the deadline ends as an expired invite (another admin would let the key back in, but the
/// string does not name one).
async fn then_redeem_is_refused(a: &RunningNode, excluded: &RunningNode) {
    let (_dealt, mut second) = deal(a).await;
    second.peers = vec![addr_of(excluded)];
    let got = redeem_via(&second.head, &second.peers, short_opts(&second.head, 4)).await;
    let error = got.expect_err("an excluded node must not hand out the key");
    assert_eq!(error, RedeemError::Expired);
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
