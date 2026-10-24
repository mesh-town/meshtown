//! Blocking nodes and actions aimed at oneself (`actions.rs`); helpers are in `actions_tests.rs`.

use super::*;

mod block {
    use super::*;

    #[tokio::test]
    async fn block_touches_own_replica_only() {
        // given: a on the list
        let world = setup(&[pid(2), pid(3)]);
        let a = pid(2);
        world.refresh().await;
        let shared_before = world.shared_entries().await;
        // when: I block a
        let changed = world.apply(MemberAction::Block(a)).await;
        // then: the block is in the own replica, the shared one has not changed, a is refused here
        assert_eq!(changed, Ok(true));
        let entry = world
            .mine(&world.own, &block_key(&a))
            .await
            .expect("the block must be in the own replica");
        assert!(!entry.is_tombstone());
        assert_eq!(
            world.shared_entries().await,
            shared_before,
            "a block must not go to the network"
        );
        assert_eq!(world.verdict(a), Err(Deny::Blocked));
    }

    #[tokio::test]
    async fn block_twice_is_unchanged() {
        // given: a is blocked
        let world = setup(&[pid(2)]);
        world.apply(MemberAction::Block(pid(2))).await.unwrap();
        // when / then: the second block changes nothing
        assert_eq!(world.apply(MemberAction::Block(pid(2))).await, Ok(false));
    }

    #[tokio::test]
    async fn unblock_removes_the_block() {
        // given: a is blocked
        let world = setup(&[pid(2)]);
        let a = pid(2);
        world.apply(MemberAction::Block(a)).await.unwrap();
        // when: unblock
        let changed = world.apply(MemberAction::Unblock(a)).await;
        // then: a passes again, and the own replica holds a tombstone
        assert_eq!(changed, Ok(true));
        assert_eq!(world.verdict(a), Ok(()));
        assert!(world
            .mine(&world.own, &block_key(&a))
            .await
            .is_some_and(|e| e.is_tombstone()));
    }

    #[tokio::test]
    async fn unblock_of_unknown_id_is_unchanged() {
        // given: nothing is blocked
        let world = setup(&[pid(2)]);
        // when / then: unblock of a node I never blocked
        assert_eq!(world.apply(MemberAction::Unblock(pid(9))).await, Ok(false));
    }

    #[tokio::test]
    async fn block_allows_unlisted_node() {
        // given: a node that is not on the list
        let world = setup(&[pid(2)]);
        let stranger = pid(9);
        // when: I block it
        let changed = world.apply(MemberAction::Block(stranger)).await;
        // then: allowed (only a vote needs a member), and the block shows
        assert_eq!(changed, Ok(true));
        assert_eq!(world.verdict(stranger), Err(Deny::Blocked));
    }
}

mod self_target {
    use super::*;

    #[tokio::test]
    async fn self_target_is_refused() {
        // given: a node with a neighbour
        let world = setup(&[pid(2)]);
        let me = world.me;
        let before = (world.shared_entries().await, world.own_entries().await);
        for act in [
            MemberAction::Exclude(me, None),
            MemberAction::Unexclude(me),
            MemberAction::Block(me),
            MemberAction::Unblock(me),
        ] {
            // when: an action names the node itself
            let result = world.apply(act.clone()).await;
            // then: refused (leaving has its own action)
            assert_eq!(result, Err(ActionRefusal::SelfTarget), "{act:?}");
        }
        assert_eq!(
            (world.shared_entries().await, world.own_entries().await),
            before,
            "nothing is written"
        );
    }
}
