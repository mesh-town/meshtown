//! Confirming and withdrawing a confirmation (`actions.rs`); helpers are in `actions_tests.rs`.
//! Nodes: `me` (seed 1) lists `a` (2), `b` (3) and `c` (4); `x` (9) asks to join; `stranger` is unknown.
//! One confirmation admits a newcomer; a request is a node that only has witness records.

use super::*;

/// A record of `author` about `subject` with the given way of writing.
fn given(world: &World, author: PeerId, kind: AclKind, subject: PeerId, via: Via) {
    let value = AclValue { kind, subject, via, made_unix: 111, note: String::new() };
    let key = crate::access::record::key_for(kind, &subject);
    world.shared.put_raw(ReplicaEntry { author, key, value: Bytes::from(encode(&value)), ts_micros: 1 });
}

/// The node with a, b listed and a witnessing x: x is a request without a counted vote.
async fn asked_by_a() -> World {
    let world = setup(&[pid(2), pid(3)]);
    given(&world, pid(2), AclKind::Admit, pid(9), Via::Witness);
    world.refresh().await;
    world
}

/// My live admit record of `subject`, if any.
async fn my_admit(world: &World, subject: PeerId) -> Option<AclValue> {
    let entry = world.mine(&world.shared, &admit_key(&subject)).await.filter(|e| !e.is_tombstone())?;
    match parse_entry(&entry) {
        Parsed::Record { value, .. } => Some(value),
        _ => None,
    }
}

#[tokio::test]
async fn admit_writes_my_vote() {
    // given: x is a request with no counted vote
    let world = asked_by_a().await;
    let x = pid(9);
    assert!(world.ledger().admissions[&x].voters.is_empty());
    // when: I confirm
    let changed = world.apply(MemberAction::Admit(x)).await;
    // then: my plain vote is stored and x joined for me
    assert_eq!(changed, Ok(true));
    assert_eq!(my_admit(&world, x).await.map(|v| v.via), Some(Via::None));
    assert_eq!(world.verdict(x), Ok(()));
    assert!(!world.ledger().admissions.contains_key(&x));
}

#[tokio::test]
async fn admit_refuses_not_pending() {
    // given: a member nobody waits for, an unknown node and an excluded one (two votes of three)
    let world = setup(&[pid(2), pid(3), pid(4)]);
    world.given_vote(pid(2), pid(4), 10);
    world.given_vote(pid(3), pid(4), 11);
    world.refresh().await;
    // when / then: none of them is a request
    for subject in [pid(2), pid(7), pid(4)] {
        let before = world.shared_entries().await;
        assert_eq!(world.apply(MemberAction::Admit(subject)).await, Err(ActionRefusal::NotPending));
        assert_eq!(world.shared_entries().await, before, "nothing is written");
    }
}

#[tokio::test]
async fn admit_refuses_when_my_vote_is_counted() {
    // given: a is listed by my vote, which already counts
    let world = held_by_my_vote().await;
    // when / then: confirming again is refused
    assert_eq!(world.apply(MemberAction::Admit(pid(2))).await, Err(ActionRefusal::NotPending));
}

#[tokio::test]
async fn admit_refuses_when_i_left() {
    // given: I have left
    let world = asked_by_a().await;
    world.apply(MemberAction::Leave).await.unwrap();
    // when / then: the refusal is `Left`, before anything else
    assert_eq!(world.apply(MemberAction::Admit(pid(9))).await, Err(ActionRefusal::Left));
    assert_eq!(world.apply(MemberAction::Unadmit(pid(9))).await, Err(ActionRefusal::Left));
}

#[tokio::test]
async fn admit_refuses_self() {
    // given: any list
    let world = asked_by_a().await;
    // when / then: the own node is a self target for both actions
    assert_eq!(world.apply(MemberAction::Admit(world.me)).await, Err(ActionRefusal::SelfTarget));
    assert_eq!(world.apply(MemberAction::Unadmit(world.me)).await, Err(ActionRefusal::SelfTarget));
}

#[tokio::test]
async fn unadmit_refuses_for_a_member_my_vote_admitted() {
    // given: a is a member by my vote alone, so there is no request to take it back from
    let world = held_by_my_vote().await;
    let a = pid(2);
    // when / then: the vote stays
    assert_eq!(world.apply(MemberAction::Unadmit(a)).await, Err(ActionRefusal::NotConfirmed));
    assert!(my_admit(&world, a).await.is_some());
}

#[tokio::test]
async fn unadmit_refuses_after_quorum() {
    // given: I confirmed x and it joined
    let world = asked_by_a().await;
    world.apply(MemberAction::Admit(pid(9))).await.unwrap();
    // when / then: there is nothing to take back any more
    assert_eq!(world.apply(MemberAction::Unadmit(pid(9))).await, Err(ActionRefusal::NotConfirmed));
    assert!(my_admit(&world, pid(9)).await.is_some(), "the vote stays");
}

#[tokio::test]
async fn unadmit_refuses_when_not_mine() {
    // given: x waits with a witness of a only
    let world = asked_by_a().await;
    // when / then: I have nothing to withdraw; an unknown node is the same
    assert_eq!(world.apply(MemberAction::Unadmit(pid(9))).await, Err(ActionRefusal::NotConfirmed));
    assert_eq!(world.apply(MemberAction::Unadmit(pid(7))).await, Err(ActionRefusal::NotConfirmed));
}

/// m lists a, b, c by plain votes; b and c only witness a. a is a member by my vote alone.
async fn held_by_my_vote() -> World {
    let world = setup(&[]);
    for p in [pid(2), pid(3), pid(4)] {
        given(&world, world.me, AclKind::Admit, p, Via::None);
    }
    for witness in [pid(3), pid(4)] {
        given(&world, witness, AclKind::Admit, pid(2), Via::Witness);
    }
    world.refresh().await;
    world
}
