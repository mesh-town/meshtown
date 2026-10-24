//! A member excluded from a Managed network comes back with a new invite of an administrator: the
//! redemption succeeds, the exclusion is lifted and the node keeps the permanent key it had.

use meshtown_core::members::access::actions::{apply, MemberAction};
use meshtown_core::transport::net::NodeSecret;

#[path = "common/invite_nodes.rs"]
mod common;
#[path = "common/managed_net.rs"]
mod managed;
#[path = "common/invite_mesh.rs"]
mod mesh;

use common::wait_until;
use managed::{
    redeem_as, start_joined, wait_both_members, wait_can_invite, ManagedNet, VIEW_LIMIT,
};
use mesh::deal;

const SEED: u8 = 5;

#[ignore = "the redemption succeeds but the creator keeps the node excluded: the access layer does not lift the exclusion yet"]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn excluded_member_returns_by_a_new_admin_invite() {
    // given: a member of the creator's network, then excluded by the creator
    let net = ManagedNet::new();
    let root = net.start_root(net.base.clone()).await;
    wait_can_invite(&root).await;
    let (_dealt, first) = deal(&root).await;
    let answer = redeem_as(SEED, &first.head, &first.peers)
        .await
        .expect("the first invite must be redeemed");
    let member = start_joined(&first.head, &answer, SEED, first.peers.clone()).await;
    wait_both_members(&member, &root).await;
    let owner = NodeSecret::from_bytes(&[SEED; 32]).peer_id();
    let excluded = apply(&root.access, MemberAction::Exclude(member.id, None)).await;
    assert_eq!(excluded, Ok(true), "the creator must exclude the member");
    assert!(
        wait_until(VIEW_LIMIT, || !root.members.access().allows(owner)).await,
        "the exclusion did not reach the creator's view"
    );
    // when: the creator issues a new invite and the excluded node redeems it under its own key
    let (_dealt, second) = deal(&root).await;
    let got = redeem_as(SEED, &second.head, &second.peers).await;
    // then: the redemption succeeds, the node is no longer excluded and its key is the same
    assert!(got.is_ok(), "the excluded member was refused: {got:?}");
    assert!(
        wait_until(VIEW_LIMIT, || root.members.access().allows(owner)).await,
        "the creator still lists the node as excluded"
    );
    assert_eq!(member.id, owner, "the node keeps its key");
    member.shutdown().await;
    root.shutdown().await;
}
