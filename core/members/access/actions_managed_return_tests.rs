//! Tests of the return of an excluded node in an administered network: a later admission by an
//! administrator lifts the exclusion for everyone, including the administrator who excluded it.

use super::support::*;
use crate::members::access::actions::MemberAction;
use crate::members::access::record::{exclude_key, AclKind};
use crate::members::access::report::members_reply;
use crate::members::access::view::Deny;
use crate::transport::net::contract::pid;
use crate::transport::net::PeerId;
use crate::web_gui::MemberRow;

const EXCLUDED_AT: u64 = 5;

#[tokio::test]
async fn the_creator_lets_in_a_node_returned_by_another_admin() {
    // given: the creator excluded the member, then another administrator admitted it
    let world = excluded_by_the_creator_then_admitted_by_another_admin().await;
    // then
    assert_eq!(world.verdict(member()), Ok(()));
}

#[tokio::test]
async fn the_own_admit_after_the_own_exclusion_lets_the_node_in() {
    // given: the creator excluded the member
    let world = setup(Seat::Root);
    world.given_exclude(EXCLUDED_AT, world.me, member());
    world.refresh().await;
    assert_eq!(world.verdict(member()), Err(Deny::Excluded));
    // when: the creator admits it again
    let changed = world.apply(MemberAction::Admit(member())).await;
    // then
    assert_eq!(changed, Ok(true));
    assert_eq!(world.verdict(member()), Ok(()));
}

#[tokio::test]
async fn an_admin_lets_in_a_node_returned_by_the_creator() {
    // given: I am an administrator and excluded the member; the creator admitted it later
    let world = setup(Seat::Admin);
    world.given_exclude(EXCLUDED_AT, world.me, member());
    world.given_admit(6, world.root, member());
    // when
    world.refresh().await;
    // then
    assert_eq!(world.verdict(member()), Ok(()));
}

#[tokio::test]
async fn an_earlier_admit_keeps_the_node_out() {
    // given: another administrator admitted the member at 4, I excluded it at 5
    let world = setup(Seat::Root);
    world.given_administrator(other_admin());
    world.given_admit(4, other_admin(), member());
    world.given_exclude(EXCLUDED_AT, world.me, member());
    // when
    world.refresh().await;
    // then
    assert_eq!(world.verdict(member()), Err(Deny::Excluded));
}

#[tokio::test]
async fn an_admit_of_a_revoked_admin_does_not_bring_the_node_back() {
    // given: the administrator lost the role at 6 and admitted the excluded member at 7
    let world = setup(Seat::Root);
    world.given_administrator(other_admin());
    world.given_exclude(EXCLUDED_AT, world.me, member());
    world.given_record(6, world.root, AclKind::Revoke, other_admin());
    world.given_admit(7, other_admin(), member());
    // when
    world.refresh().await;
    // then
    assert_eq!(world.verdict(member()), Err(Deny::Excluded));
    assert!(
        !world.ledger().is_member(&member()),
        "the node must not be a member"
    );
}

#[tokio::test]
async fn the_exclusion_of_a_revoked_admin_stands_until_a_later_admit() {
    // given: I am an administrator, excluded the member at 5 and lost the role at 6
    let world = setup(Seat::Admin);
    world.given_exclude(EXCLUDED_AT, world.me, member());
    world.given_record(6, world.root, AclKind::Revoke, world.me);
    world.refresh().await;
    assert_eq!(world.verdict(member()), Err(Deny::Excluded));
    // when: the creator admits the member at 7
    world.given_admit(7, world.root, member());
    world.refresh().await;
    // then
    assert_eq!(world.verdict(member()), Ok(()));
}

#[tokio::test]
async fn excluding_again_after_the_return_keeps_the_node_out() {
    // given: I excluded the member at 5, then admitted it back at 6; my old exclusion record is alive
    let world = returned_by_the_creator().await;
    assert_eq!(world.verdict(member()), Ok(()));
    // when: I exclude it again
    let changed = world.apply(MemberAction::Exclude(member(), None)).await;
    // then: the record is written anew, later than the admission, and the node is out
    assert_eq!(changed, Ok(true));
    let written = world.live_ts(&exclude_key(&member())).await;
    assert!(
        written > Some(6_000_000),
        "the record must be newer than the admission: {written:?}"
    );
    assert!(world.ledger().excluded.contains_key(&member()));
    assert_eq!(world.verdict(member()), Err(Deny::Excluded));
}

#[tokio::test]
async fn excluding_an_excluded_node_changes_nothing() {
    // given: I excluded the member
    let world = setup(Seat::Root);
    world
        .apply(MemberAction::Exclude(member(), None))
        .await
        .unwrap();
    let (before, count) = (
        world.live_ts(&exclude_key(&member())).await,
        world.shared_entries().await.len(),
    );
    // when: I exclude it once more
    let changed = world.apply(MemberAction::Exclude(member(), None)).await;
    // then: nothing is written
    assert_eq!(changed, Ok(false));
    assert_eq!(world.live_ts(&exclude_key(&member())).await, before);
    assert_eq!(world.shared_entries().await.len(), count);
}

#[tokio::test]
async fn unexclude_after_the_return_keeps_the_node_in() {
    // given: the member returned by my admission; my old exclusion record is alive
    let world = returned_by_the_creator().await;
    // when: I take the exclusion back
    let changed = world.apply(MemberAction::Unexclude(member())).await;
    // then: the record is gone and the node stays a member
    assert_eq!(changed, Ok(true));
    assert!(world
        .live_of(world.me, &exclude_key(&member()))
        .await
        .is_none());
    assert!(
        world.ledger().is_member(&member()),
        "the node must stay a member"
    );
    assert_eq!(world.verdict(member()), Ok(()));
}

#[tokio::test]
async fn in_a_peer_network_a_later_admit_does_not_bring_the_node_back() {
    // given: a network of two; I excluded the member at 5 and admitted it again at 6
    let world = setup_peer();
    world.given_exclude(EXCLUDED_AT, world.me, member());
    world.given_admit(6, world.me, member());
    // when
    world.refresh().await;
    // then: the exclusion is stronger than the later admission
    assert_eq!(world.verdict(member()), Err(Deny::Excluded));
}

#[tokio::test]
async fn in_a_peer_network_the_own_vote_still_refuses_after_an_admit() {
    // given: three on the list, so one vote only proposes; my vote at 5, another member's admit at 6
    let world = setup_peer();
    world.given_admit(1, world.me, pid(5));
    world.given_exclude(EXCLUDED_AT, world.me, member());
    world.given_admit(6, pid(5), member());
    // when
    world.refresh().await;
    // then
    assert_eq!(world.verdict(member()), Err(Deny::VotedByMe));
}

#[tokio::test]
async fn the_returned_node_row_has_no_own_refusal() {
    // given: the creator excluded the member, another administrator admitted it
    let world = excluded_by_the_creator_then_admitted_by_another_admin().await;
    // when
    let row = row_of(&world, member());
    // then
    assert_eq!(row.status, "member");
    assert_eq!(row.denied_by_me, None);
}

/// I am the creator; the member was excluded by me at 5 and admitted by another administrator at 6.
async fn excluded_by_the_creator_then_admitted_by_another_admin() -> World {
    let world = setup(Seat::Root);
    world.given_administrator(other_admin());
    world.given_exclude(EXCLUDED_AT, world.me, member());
    world.given_admit(6, other_admin(), member());
    world.refresh().await;
    world
}

/// I am the creator; the member was excluded by me at 5 and admitted by me at 6.
async fn returned_by_the_creator() -> World {
    let world = setup(Seat::Root);
    world.given_exclude(EXCLUDED_AT, world.me, member());
    world.given_admit(6, world.me, member());
    world.refresh().await;
    world
}

fn row_of(world: &World, peer: PeerId) -> MemberRow {
    let verdicts = world
        .access
        .view
        .verdicts()
        .expect("the view must be published");
    members_reply(&verdicts, &world.access.members, &[], None)
        .members
        .into_iter()
        .find(|r| r.id_full == peer.to_string())
        .expect("the row must exist")
}
