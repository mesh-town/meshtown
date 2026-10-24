//! Forgetting a node again after it returned, and taking a forget back (`actions.rs`); fixtures are in
//! `actions_forget_tests.rs`.

use super::*;

mod forget_again {
    use super::*;

    #[tokio::test]
    async fn forget_again_after_return_rewrites() {
        // given: a was forgotten and then admitted again one second later
        let me = setup(&[pid(2)]);
        let a = pid(2);
        me.apply(MemberAction::Forget(a)).await.unwrap();
        let first = me.my_forget(a).await.unwrap().made_unix;
        me.given_admit(me.me, a, first + 1, 2);
        me.refresh().await;
        assert_eq!(me.verdict(a), Ok(()), "the admit must bring a back");
        // when: I vote to forget it again
        let changed = me.apply(MemberAction::Forget(a)).await;
        // then: the record is rewritten with a time after the admit and a is forgotten again
        assert_eq!(changed, Ok(true));
        assert!(me.my_forget(a).await.unwrap().made_unix > first + 1);
        assert_eq!(me.verdict(a), Err(Deny::Forgotten));
    }

    #[tokio::test]
    async fn forget_again_steps_over_future_admit() {
        // given: the latest admit of b is stamped far ahead of my clock
        let me = setup(&[pid(2), pid(3)]);
        let b = pid(3);
        me.given_admit(me.me, b, FUTURE, 2);
        me.refresh().await;
        // when: I vote to forget b
        let changed = me.apply(MemberAction::Forget(b)).await;
        // then: the vote is stamped one second after that admit, or it would be older than the admit
        assert_eq!(changed, Ok(true));
        assert_eq!(me.my_forget(b).await.unwrap().made_unix, FUTURE + 1);
        assert_eq!(me.verdict(b), Err(Deny::RemovedByMe));
    }
}

mod unforget {
    use super::*;

    #[tokio::test]
    async fn unforget_withdraws_own_record() {
        // given: my vote to forget b
        let me = setup(&[pid(2), pid(3)]);
        let b = pid(3);
        me.apply(MemberAction::Forget(b)).await.unwrap();
        // when: I take it back
        let changed = me.apply(MemberAction::Unforget(b)).await;
        // then: the record is a tombstone, the circle is gone and b passes again
        assert_eq!(changed, Ok(true));
        let entries = me.shared.scan(&forget_key(&b)).await.unwrap();
        let entry = entries.into_iter().find(|e| e.author == me.me);
        assert!(
            entry.expect("the key must keep a tombstone").is_tombstone(),
            "the vote must be withdrawn, not deleted"
        );
        assert!(!me.ledger().forgets.contains_key(&b));
        assert_eq!(me.verdict(b), Ok(()));
    }

    #[tokio::test]
    async fn unforget_without_record_is_not_forgotten() {
        let me = setup(&[pid(2), pid(3)]);
        // when / then: there is nothing to take back, nothing is written
        let before = me.entries().await;
        assert_eq!(
            me.apply(MemberAction::Unforget(pid(3))).await,
            Err(ActionRefusal::NotForgotten)
        );
        assert_eq!(me.entries().await, before);
    }

    #[tokio::test]
    async fn unforget_does_not_touch_the_votes_of_others() {
        // given: only a has voted to forget b
        let me = setup(&[pid(2), pid(3)]);
        let (a, b) = (pid(2), pid(3));
        me.given(a, AclKind::Forget, b, 200, 2);
        me.refresh().await;
        // when / then: my take-back is refused, a's vote stays in the circle
        assert_eq!(
            me.apply(MemberAction::Unforget(b)).await,
            Err(ActionRefusal::NotForgotten)
        );
        assert_eq!(me.ledger().forgets[&b].voters, vec![a]);
    }

    #[tokio::test]
    async fn unforget_one_of_two_votes_returns_member() {
        // given: b is forgotten by my vote and the vote of a
        let me = setup(&[pid(2), pid(3)]);
        let (a, b) = (me.peer(2, &[me.me, pid(3)]), pid(3));
        me.apply(MemberAction::Forget(b)).await.unwrap();
        a.pull_from(&me).await;
        a.apply(MemberAction::Forget(b)).await.unwrap();
        me.pull_from(&a).await;
        me.refresh().await;
        assert_eq!(me.verdict(b), Err(Deny::Forgotten));
        // when: I take my vote back
        let changed = me.apply(MemberAction::Unforget(b)).await;
        // then: one vote is below the threshold, b is a member again and a's vote waits
        assert_eq!(changed, Ok(true));
        assert_eq!(me.verdict(b), Ok(()));
        assert_eq!(me.ledger().forgets[&b].voters, vec![a.me]);
        assert_eq!(me.ledger().forgets[&b].at_unix, None);
    }
}
