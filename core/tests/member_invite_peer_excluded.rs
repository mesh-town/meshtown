//! A node excluded from a peer network is refused when it redeems a new invite, and the refusal says
//! that the computer can't rejoin. An exclusion in a peer network is not lifted by a later invite.

use meshtown_core::invite::redeem::RedeemError;
use meshtown_core::members::access::actions::{apply, MemberAction};
use meshtown_core::members::network_key::NetworkKey;
use meshtown_core::node::{self, NodeConfig, RunningNode};
use meshtown_core::transport::net::{NodeSecret, PeerAddr};

#[path = "common/invite_nodes.rs"]
mod common;
#[path = "common/invite_mesh.rs"]
mod mesh;
use common::{addr_of, quick_opts, redeem_via, wait_until, ENTRY_LIMIT_SECS, OWNER_SEED};
use mesh::deal;

/// The ceiling of one wait for a membership change.
const LIMIT: std::time::Duration = std::time::Duration::from_secs(20);

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_excluded_peer_is_refused_by_a_new_invite() {
    // given: a network of two, the second joined by an invite and then excluded by the first
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
    let excluded = start_listed(&granted, Some(owner.clone()), first.peers.clone()).await;
    let both = || a.members.is_member(excluded.id) && excluded.members.is_member(a.id);
    assert!(
        wait_until(LIMIT, both).await,
        "the nodes did not become members within {LIMIT:?}"
    );
    let voted = apply(&a.access, MemberAction::Exclude(excluded.id, None)).await;
    assert_eq!(
        voted,
        Ok(true),
        "one vote must exclude the node in a network of two"
    );
    let refused = || !a.members.access().allows(owner.peer_id());
    assert!(
        wait_until(LIMIT, refused).await,
        "the exclusion did not reach the first node"
    );
    // when: the excluded computer, no longer running as a node, redeems a new invite under its own key
    excluded.shutdown().await;
    let (_dealt, second) = deal(&a).await;
    let got = redeem_via(
        &second.head,
        &second.peers,
        quick_opts(&second.head, ENTRY_LIMIT_SECS),
    )
    .await;
    // then: it is refused for good, with the text for a computer that can't rejoin
    let error = got.expect_err("an excluded node must not be let in by a new invite");
    assert_eq!(error, RedeemError::Excluded);
    assert_eq!(
        error.to_string(),
        "This computer can't rejoin this network. The invite wasn't used up — someone else can join with it."
    );
    a.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_running_excluded_node_does_not_hand_out_a_key() {
    // given: a network of two, the second joined by an invite and then excluded by the first
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
    let excluded = start_listed(&granted, Some(owner.clone()), first.peers.clone()).await;
    let both = || a.members.is_member(excluded.id) && excluded.members.is_member(a.id);
    assert!(
        wait_until(LIMIT, both).await,
        "the nodes did not become members within {LIMIT:?}"
    );
    let voted = apply(&a.access, MemberAction::Exclude(excluded.id, None)).await;
    assert_eq!(voted, Ok(true), "one vote must exclude the node");
    let informed =
        || !a.members.access().allows(owner.peer_id()) && !excluded.members.access().allows(a.id);
    assert!(
        wait_until(LIMIT, informed).await,
        "the exclusion did not reach both nodes"
    );
    // when: the excluded node keeps running, listed first among the addresses of a new invite,
    // and its computer redeems the invite under its own key
    let (_dealt, mut second) = deal(&a).await;
    second.peers = vec![addr_of(&excluded), addr_of(&a)];
    let got = redeem_via(
        &second.head,
        &second.peers,
        quick_opts(&second.head, ENTRY_LIMIT_SECS),
    )
    .await;
    // then: the running excluded node gives no key; the first node refuses for good
    let error = got.expect_err("an excluded node must not hand out the key");
    assert_eq!(error, RedeemError::Excluded);
    excluded.shutdown().await;
    a.shutdown().await;
}

/// A node that consults the membership list, with no SOCKS listener.
async fn start_listed(
    key: &NetworkKey,
    owner: Option<NodeSecret>,
    peers: Vec<PeerAddr>,
) -> RunningNode {
    let mut cfg = NodeConfig::for_loopback_test(key.clone());
    cfg.open_list = false;
    cfg.identity_secret = owner;
    cfg.initial_peers = peers;
    node::start_node(cfg).await.unwrap()
}
