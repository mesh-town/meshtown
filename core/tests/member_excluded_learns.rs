//! A member excluded while its node was off learns it on the next start from the refusals of its neighbours:
//! the report says the node is excluded, not merely refused. The in-memory transport carries no close
//! reasons, so the neighbour's refusal is noted by hand, the way the handshake notes it on a real transport.

use meshtown_core::members::access::actions::{apply, MemberAction};
use meshtown_core::members::access::report::members_reply;
use meshtown_core::members::access::view::{Deny, Side};

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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_node_excluded_while_off_learns_it_from_refusals() {
    // given: a member that went off, then was excluded by the creator
    let net = ManagedNet::new();
    let root = net.start_root(net.base.clone()).await;
    wait_can_invite(&root).await;
    let (_dealt, invite) = deal(&root).await;
    let answer = redeem_as(SEED, &invite.head, &invite.peers)
        .await
        .expect("the invite must be redeemed");
    let member = start_joined(&invite.head, &answer, SEED, invite.peers.clone()).await;
    wait_both_members(&member, &root).await;
    let id = member.id;
    member.shutdown().await;
    let excluded = apply(&root.access, MemberAction::Exclude(id, None)).await;
    assert_eq!(excluded, Ok(true), "the creator must exclude the member");
    // when: the node starts again under the same key and the creator turns it away as excluded
    let back = start_joined(&invite.head, &answer, SEED, invite.peers.clone()).await;
    back.members
        .access()
        .note_refusal(root.id, Side::Them, Deny::Excluded);
    // then: its report says it is excluded and not just waiting for admission
    let learned = || {
        let Some(v) = back.members.access().verdicts() else {
            return false;
        };
        let reply = members_reply(
            &v,
            &back.members,
            "",
            &back.members.access().refusals(),
            None,
        );
        reply.me_excluded && !reply.me_pending
    };
    assert!(
        wait_until(VIEW_LIMIT, learned).await,
        "the node did not learn it is excluded within {VIEW_LIMIT:?}: {:?}",
        back.members.access().refusals()
    );
    back.shutdown().await;
    root.shutdown().await;
}
