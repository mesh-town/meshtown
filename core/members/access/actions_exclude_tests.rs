//! Excluding and unexcluding members (`actions.rs`); helpers are in `actions_tests.rs`.

use super::*;

mod exclude {
    use super::*;

    #[tokio::test]
    async fn exclude_writes_vote_and_denies_locally() {
        // given: me, a and b on the list: two votes are needed to exclude
        let world = setup(&[pid(2), pid(3)]);
        let a = pid(2);
        // when: I vote to exclude a
        let changed = world.apply(MemberAction::Exclude(a, None)).await;
        // then: my vote is in the shared replica, it works at once for me, but a is not excluded
        assert_eq!(changed, Ok(true));
        assert_eq!(world.vote_subjects().await, vec![a], "my votes");
        assert_eq!(world.verdict(a), Err(Deny::VotedByMe));
        assert_eq!(
            world.ledger().pending[&a].voters,
            vec![world.me],
            "the proposal is mine"
        );
        assert!(
            !world.ledger().excluded.contains_key(&a),
            "one vote is not enough yet"
        );
        assert_eq!(
            world.verdict(pid(3)),
            Ok(()),
            "the other neighbour is untouched"
        );
    }

    #[tokio::test]
    async fn exclude_twice_is_unchanged() {
        // given: my vote against a is cast
        let world = setup(&[pid(2), pid(3)]);
        let a = pid(2);
        world.apply(MemberAction::Exclude(a, None)).await.unwrap();
        let stored = world.mine(&world.shared, &exclude_key(&a)).await;
        // when: the same action again
        let again = world.apply(MemberAction::Exclude(a, None)).await;
        // then: nothing changed, the record is the same one
        assert_eq!(again, Ok(false));
        assert_eq!(
            world.mine(&world.shared, &exclude_key(&a)).await,
            stored,
            "the vote was rewritten"
        );
    }

    #[tokio::test]
    async fn exclude_confirming_a_proposal_excludes_the_node() {
        // given: four on the list; b has already voted against a
        let world = setup(&[pid(2), pid(3), pid(4)]);
        let (a, b) = (pid(2), pid(3));
        world.given_vote(b, a, 100);
        world.refresh().await;
        assert_eq!(
            world.verdict(a),
            Ok(()),
            "one vote of four is a proposal only"
        );
        // when: I confirm
        let changed = world.apply(MemberAction::Exclude(a, None)).await;
        // then: a is excluded by the two votes, in time order
        assert_eq!(changed, Ok(true));
        assert_eq!(world.verdict(a), Err(Deny::Excluded));
        assert_eq!(world.ledger().excluded[&a].voters, vec![b, world.me]);
    }

    #[tokio::test]
    async fn exclude_in_a_network_of_two_excludes_at_once() {
        // given: only me and a
        let world = setup(&[pid(2)]);
        let a = pid(2);
        // when: I vote against a
        let changed = world.apply(MemberAction::Exclude(a, None)).await;
        // then: one vote is the threshold for two nodes
        assert_eq!(changed, Ok(true));
        assert_eq!(world.verdict(a), Err(Deny::Excluded));
        assert_eq!(world.ledger().excluded[&a].how, How::Votes);
    }

    #[tokio::test]
    async fn exclude_unlisted_is_not_member() {
        // given: a node that is not on the list
        let world = setup(&[pid(2)]);
        let stranger = pid(9);
        let before = world.shared_entries().await;
        // when: I vote to exclude it
        let result = world.apply(MemberAction::Exclude(stranger, None)).await;
        // then: refused, nothing is written
        assert_eq!(result, Err(ActionRefusal::NotMember));
        assert_eq!(world.shared_entries().await, before);
    }
}

mod unexclude {
    use super::*;

    #[tokio::test]
    async fn unexclude_tombstones_vote() {
        // given: my vote against a
        let world = setup(&[pid(2), pid(3)]);
        let a = pid(2);
        world.apply(MemberAction::Exclude(a, None)).await.unwrap();
        // when: I take it back
        let changed = world.apply(MemberAction::Unexclude(a)).await;
        // then: the vote is a tombstone and a passes again
        assert_eq!(changed, Ok(true));
        let entry = world
            .mine(&world.shared, &exclude_key(&a))
            .await
            .expect("the key must keep a tombstone");
        assert!(
            entry.is_tombstone(),
            "the vote must be withdrawn, not deleted or rewritten"
        );
        assert_eq!(world.verdict(a), Ok(()));
    }

    #[tokio::test]
    async fn unexclude_without_vote_is_unchanged() {
        // given: no vote of mine
        let world = setup(&[pid(2), pid(3)]);
        let before = world.shared_entries().await;
        // when: unexclude
        let changed = world.apply(MemberAction::Unexclude(pid(2))).await;
        // then: nothing changed and nothing is written
        assert_eq!(changed, Ok(false));
        assert_eq!(world.shared_entries().await, before);
    }

    #[tokio::test]
    async fn unexclude_returns_node_held_by_my_vote_alone() {
        // given: me and a only: my single vote has excluded a
        let world = setup(&[pid(2)]);
        let a = pid(2);
        world.apply(MemberAction::Exclude(a, None)).await.unwrap();
        assert_eq!(world.verdict(a), Err(Deny::Excluded));
        // when: I take the vote back
        let changed = world.apply(MemberAction::Unexclude(a)).await;
        // then: a is a member again
        assert_eq!(changed, Ok(true));
        assert_eq!(world.verdict(a), Ok(()));
        assert!(world.ledger().excluded.is_empty(), "nobody is excluded");
    }
}

mod vote_at_moment {
    use super::*;

    #[tokio::test]
    async fn exclude_writes_a_vote_at_moment() {
        // given: me, a and b on the list
        let world = setup(&[pid(2), pid(3)]);
        let a = pid(2);
        // when: I vote to exclude a
        world.apply(MemberAction::Exclude(a, None)).await.unwrap();
        // then: my record carries the moment mark
        assert_eq!(my_vote(&world, a).await.via, VOTE_AT_MOMENT);
    }

    #[tokio::test]
    async fn exclude_keeps_an_old_unmarked_vote() {
        // given: three on the list; my old vote against a has no mark
        let world = setup(&[pid(2), pid(3)]);
        let a = pid(2);
        world.given_vote(world.me, a, 100);
        world.refresh().await;
        // when: I vote against a again
        let changed = world.apply(MemberAction::Exclude(a, None)).await;
        // then: nothing is rewritten, the old record stays without the mark
        assert_eq!(changed, Ok(false));
        assert_eq!(my_vote(&world, a).await.via, Via::Invite);
    }
}

mod after_growth {
    use super::*;

    #[tokio::test]
    async fn exclusion_in_two_survives_a_later_invite() {
        // given: me and b; my vote has excluded b, then c is admitted
        let world = excluded_in_two_then_a_newcomer().await;
        let (b, c) = (pid(2), pid(3));
        // then: b stays excluded, it is not just my pending vote; c is a member
        assert_eq!(world.verdict(b), Err(Deny::Excluded));
        assert!(world.ledger().is_member(&c));
    }

    #[tokio::test]
    async fn unexclude_after_growth_returns_node() {
        // given: b is excluded by my vote alone in a network that has grown to three
        let world = excluded_in_two_then_a_newcomer().await;
        let b = pid(2);
        // when: I take the vote back
        let changed = world.apply(MemberAction::Unexclude(b)).await;
        // then: b returns
        assert_eq!(changed, Ok(true));
        assert_eq!(world.verdict(b), Ok(()));
    }

    #[tokio::test]
    async fn exclude_again_after_growth_needs_two_votes() {
        // given: my vote at 200 s has excluded b in a network of two; c was admitted at 300 s
        let world = setup(&[pid(2)]);
        let (b, c) = (pid(2), pid(3));
        world.given_vote_at_moment(world.me, b, 200);
        world.given_admit_at(world.me, c, 300);
        world.refresh().await;
        assert_eq!(world.verdict(b), Err(Deny::Excluded));
        // when: I take the vote back and vote again now
        world.apply(MemberAction::Unexclude(b)).await.unwrap();
        let changed = world.apply(MemberAction::Exclude(b, None)).await;
        // then: three are active, so one vote only waits for the second
        assert_eq!(changed, Ok(true));
        assert_eq!(world.verdict(b), Err(Deny::VotedByMe));
        assert_eq!(world.ledger().pending[&b].voters, vec![world.me]);
    }
}

// --- helpers ---

/// Me and b; I exclude b alone, then c is admitted a minute from now.
async fn excluded_in_two_then_a_newcomer() -> World {
    let world = setup(&[pid(2)]);
    world
        .apply(MemberAction::Exclude(pid(2), None))
        .await
        .unwrap();
    world.given_admit_at(world.me, pid(3), unix_now() + 60);
    world.refresh().await;
    world
}

/// The decoded record of my own exclusion vote against `subject`.
async fn my_vote(world: &World, subject: PeerId) -> AclValue {
    let entry = world.mine(&world.shared, &exclude_key(&subject)).await;
    match parse_entry(&entry.expect("my vote must be stored")) {
        Parsed::Record { value, .. } => value,
        other => panic!("not a record: {other:?}"),
    }
}

fn unix_now() -> u64 {
    let now = std::time::SystemTime::now();
    now.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
}
