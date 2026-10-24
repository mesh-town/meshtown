//! A member excluded from a Managed network comes back with a new invite of an administrator: the
//! redemption succeeds, the exclusion is lifted and the node keeps the permanent key it had.

use meshtown_core::members::access::actions::{apply, MemberAction};
use meshtown_core::node::RunningNode;
use meshtown_core::transport::net::{NodeSecret, PeerId};

#[path = "common/invite_nodes.rs"]
mod common;
#[path = "common/managed_net.rs"]
mod managed;
#[path = "common/invite_mesh.rs"]
mod mesh;

use common::{addr_of, wait_until};
use managed::{
    redeem_as, start_joined, wait_both_members, wait_can_invite, ManagedNet, VIEW_LIMIT,
};
use mesh::deal;

const SEED: u8 = 5;
const ADMIN_SEED: u8 = 6;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn excluded_member_returns_by_a_new_admin_invite() {
    // given: a member of the creator's network, then excluded by the creator
    let net = ManagedNet::new();
    let root = net.start_root(net.base.clone()).await;
    wait_can_invite(&root).await;
    let member = join(&root, SEED).await;
    let owner = member.id;
    exclude(&root, owner).await;
    wait_listed(&[&root], owner, false).await;
    // when: the creator issues a new invite and the excluded node redeems it under its own key
    let (_dealt, second) = deal(&root).await;
    let got = redeem_as(SEED, &second.head, &second.peers).await;
    // then: the redemption succeeds, the node is no longer excluded and its key is the same
    assert!(got.is_ok(), "the excluded member was refused: {got:?}");
    wait_listed(&[&root], owner, true).await;
    assert_eq!(
        owner,
        NodeSecret::from_bytes(&[SEED; 32]).peer_id(),
        "the node keeps its key"
    );
    member.shutdown().await;
    root.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_admin_who_excluded_lets_the_returned_node_in() {
    // given: the member was excluded by an administrator who is not the creator
    let (_net, root, admin, member) = admins_network().await;
    exclude(&admin, member.id).await;
    wait_listed(&[&root, &admin], member.id, false).await;
    // when: the node redeems a new invite of the creator
    let (_dealt, invite) = deal(&root).await;
    let got = redeem_as(SEED, &invite.head, &invite.peers).await;
    // then: the administrator who excluded it lets it in again
    assert!(got.is_ok(), "the excluded member was refused: {got:?}");
    wait_listed(&[&admin], member.id, true).await;
    shut_down([member, admin, root]).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_invite_of_the_admin_who_excluded_brings_the_node_back() {
    // given: the member was excluded by an administrator who is not the creator
    let (_net, root, admin, member) = admins_network().await;
    exclude(&admin, member.id).await;
    wait_listed(&[&root, &admin], member.id, false).await;
    // when: the node redeems a new invite of that administrator, reaching only him
    let (_dealt, invite) = deal(&admin).await;
    let got = redeem_as(SEED, &invite.head, &[addr_of(&admin)]).await;
    // then: the administrator lets it in again
    assert!(got.is_ok(), "the excluded member was refused: {got:?}");
    wait_listed(&[&admin], member.id, true).await;
    shut_down([member, admin, root]).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_invite_dealt_before_the_exclusion_brings_the_node_back() {
    // given: the creator dealt an invite, then excluded the member
    let net = ManagedNet::new();
    let root = net.start_root(net.base.clone()).await;
    wait_can_invite(&root).await;
    let member = join(&root, SEED).await;
    let (_dealt, early) = deal(&root).await;
    exclude(&root, member.id).await;
    wait_listed(&[&root], member.id, false).await;
    // when: the excluded node redeems the invite dealt before the exclusion
    let got = redeem_as(SEED, &early.head, &early.peers).await;
    // then: the creator lets it in again
    assert!(got.is_ok(), "the excluded member was refused: {got:?}");
    wait_listed(&[&root], member.id, true).await;
    shut_down([member, root]).await;
}

/// The creator, an administrator granted the role, and a plain member; the administrator lists the member.
async fn admins_network() -> (ManagedNet, RunningNode, RunningNode, RunningNode) {
    let net = ManagedNet::new();
    let root = net.start_root(net.base.clone()).await;
    wait_can_invite(&root).await;
    let admin = join(&root, ADMIN_SEED).await;
    let granted = apply(&root.access, MemberAction::Grant(admin.id)).await;
    assert_eq!(
        granted,
        Ok(true),
        "the creator must make the node an administrator"
    );
    wait_can_invite(&admin).await;
    let member = join(&root, SEED).await;
    wait_listed(&[&admin], member.id, true).await;
    (net, root, admin, member)
}

/// A node that redeemed a new invite of `host` under the permanent key of `seed` and became its member.
async fn join(host: &RunningNode, seed: u8) -> RunningNode {
    let (_dealt, invite) = deal(host).await;
    let answer = redeem_as(seed, &invite.head, &invite.peers)
        .await
        .expect("the invite must be redeemed");
    let node = start_joined(&invite.head, &answer, seed, invite.peers.clone()).await;
    wait_both_members(&node, host).await;
    node
}

async fn exclude(by: &RunningNode, who: PeerId) {
    let excluded = apply(&by.access, MemberAction::Exclude(who, None)).await;
    assert_eq!(
        excluded,
        Ok(true),
        "the administrator must exclude the member"
    );
}

/// Waits until every node's list allows `who` (or, with `listed` false, refuses it).
async fn wait_listed(nodes: &[&RunningNode], who: PeerId, listed: bool) {
    let settled = || {
        nodes
            .iter()
            .all(|n| n.members.access().allows(who) == listed)
    };
    assert!(
        wait_until(VIEW_LIMIT, settled).await,
        "the lists did not settle (allowed: {listed}) within {VIEW_LIMIT:?}"
    );
}

async fn shut_down<const N: usize>(nodes: [RunningNode; N]) {
    for node in nodes {
        node.shutdown().await;
    }
}
