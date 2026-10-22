//! Leaving the network and the failure of a write (`actions.rs`); helpers are in `actions_tests.rs`.

use super::*;

mod leave {
    use super::*;

    #[tokio::test]
    async fn leave_excludes_me_for_everyone() {
        // given: me with two neighbours
        let world = setup(&[pid(2), pid(3)]);
        // when: I leave
        let changed = world.apply(MemberAction::Leave).await;
        // then: my self-exclusion is written, I am excluded as "left", and nobody passes
        assert_eq!(changed, Ok(true));
        assert!(world.mine(&world.shared, &exclude_key(&world.me)).await.is_some_and(|e| !e.is_tombstone()));
        assert_eq!(world.ledger().excluded[&world.me].how, How::Left);
        assert_eq!(world.verdict(pid(2)), Err(Deny::Left));
        assert_eq!(world.verdict(pid(3)), Err(Deny::Left));
    }

    #[tokio::test]
    async fn leave_twice_is_unchanged() {
        // given: I have left
        let world = setup(&[pid(2)]);
        world.apply(MemberAction::Leave).await.unwrap();
        // when / then: the repeat is not an error and not a change
        assert_eq!(world.apply(MemberAction::Leave).await, Ok(false));
    }

    #[tokio::test]
    async fn actions_after_leave_are_refused() {
        // given: I have left; a is listed, the stranger is not
        let world = setup(&[pid(2)]);
        let (a, stranger, me) = (pid(2), pid(9), world.me);
        world.apply(MemberAction::Leave).await.unwrap();
        let before = (world.shared_entries().await, world.own_entries().await);
        for act in [
            MemberAction::Exclude(a, None),
            MemberAction::Unexclude(a),
            MemberAction::Block(a),
            MemberAction::Unblock(a),
            MemberAction::Exclude(me, None),
            MemberAction::Exclude(stranger, None),
        ] {
            // when: any action but leave
            let result = world.apply(act.clone()).await;
            // then: refused as "left", also where another reason would apply
            assert_eq!(result, Err(ActionRefusal::Left), "{act:?}");
        }
        assert_eq!((world.shared_entries().await, world.own_entries().await), before, "nothing is written");
    }
}

mod write_failure {
    use super::*;

    #[tokio::test]
    async fn write_failure_is_reported() {
        // given: a shared replica that refuses writes
        let world = setup_with_failing_shared(&[pid(2)]);
        let before = world.shared_entries().await;
        world.fail_writes.store(true, Ordering::SeqCst);
        // when: I vote to exclude a
        let result = world.apply(MemberAction::Exclude(pid(2), None)).await;
        // then: the refusal carries the error text, and nothing was stored
        assert!(matches!(&result, Err(ActionRefusal::Write(text)) if !text.is_empty()), "{result:?}");
        assert_eq!(world.shared_entries().await, before);
    }
}
