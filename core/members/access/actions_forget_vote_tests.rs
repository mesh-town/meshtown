//! The forget action and its refusals (`actions.rs`); fixtures are in `actions_forget_tests.rs`.

use super::*;

mod forget {
    use super::*;

    #[tokio::test]
    async fn forget_offline_member_writes_record() {
        // given: three active nodes, b is not connected
        let me = setup(&[pid(2), pid(3)]);
        let b = pid(3);
        let before = unix_now();
        // when: I vote to forget b
        let changed = me.apply(MemberAction::Forget(b)).await;
        // then: my forget record about b is stored, stamped with the current time and no origin or note
        assert_eq!(changed, Ok(true));
        let value = me.my_forget(b).await.expect("my vote must be stored");
        assert_eq!(
            (value.kind, value.subject, value.via),
            (AclKind::Forget, b, Via::None)
        );
        assert_eq!(value.note, "");
        assert!(
            (before..=unix_now()).contains(&value.made_unix),
            "the vote must carry the current time"
        );
    }

    #[tokio::test]
    async fn first_vote_waits_for_a_second_and_hides_the_node_from_me() {
        // given / when: one of two needed votes
        let me = setup(&[pid(2), pid(3)]);
        let b = pid(3);
        me.apply(MemberAction::Forget(b)).await.unwrap();
        // then: the circle holds my vote only, b is not forgotten yet but denied for me
        let ledger = me.ledger();
        assert_eq!(ledger.forgets[&b].voters, vec![me.me]);
        assert_eq!(ledger.forgets[&b].at_unix, None);
        assert!(ledger.is_member(&b), "one vote of two forgets nobody");
        assert_eq!(me.verdict(b), Err(Deny::RemovedByMe));
    }

    #[tokio::test]
    async fn forget_twice_in_round_is_noop() {
        // given: my vote about b is in the current circle
        let me = setup(&[pid(2), pid(3)]);
        let b = pid(3);
        me.apply(MemberAction::Forget(b)).await.unwrap();
        let before = me.entries().await;
        // when: the same action again
        let again = me.apply(MemberAction::Forget(b)).await;
        // then: nothing is written, the record stays as it was
        assert_eq!(again, Ok(false));
        assert_eq!(me.entries().await, before, "the vote was rewritten");
    }

    #[tokio::test]
    async fn confirm_by_second_member_forgets() {
        // given: I voted to forget b and the record reached member a
        let me = setup(&[pid(2), pid(3)]);
        let (a, b) = (me.peer(2, &[me.me, pid(3)]), pid(3));
        me.apply(MemberAction::Forget(b)).await.unwrap();
        a.pull_from(&me).await;
        // when: a confirms through its own node and its record comes back to me
        let changed = a.apply(MemberAction::Forget(b)).await;
        me.pull_from(&a).await;
        me.refresh().await;
        // then: b is forgotten by both votes (same second: the order is by id)
        assert_eq!(changed, Ok(true));
        let mut voters = me.ledger().forgets[&b].voters.clone();
        voters.sort();
        assert_eq!(voters, [me.me.min(a.me), me.me.max(a.me)]);
        assert!(me.ledger().is_forgotten(&b));
        assert_eq!(me.verdict(b), Err(Deny::Forgotten));
        assert_eq!(me.verdict(a.me), Ok(()), "a stays a member");
    }
}

mod refusals {
    use super::*;

    #[tokio::test]
    async fn forget_online_member_is_refused() {
        // given: b is connected right now
        let me = setup(&[pid(2), pid(3)]);
        let b = pid(3);
        me.go_online(b);
        let before = me.entries().await;
        // when: I vote to forget it
        let result = me.apply(MemberAction::Forget(b)).await;
        // then: refused, nothing is written
        assert_eq!(result, Err(ActionRefusal::Online));
        assert_eq!(me.entries().await, before);
    }

    #[tokio::test]
    async fn forget_self_is_refused() {
        let me = setup(&[pid(2), pid(3)]);
        let result = me.apply(MemberAction::Forget(me.me)).await;
        assert_eq!(result, Err(ActionRefusal::SelfTarget));
        assert!(me.my_forget(me.me).await.is_none());
    }

    #[tokio::test]
    async fn forget_unlisted_is_not_member() {
        // given: a node that is not on the list
        let me = setup(&[pid(2)]);
        let before = me.entries().await;
        // when / then: refused, nothing is written
        let result = me.apply(MemberAction::Forget(pid(9))).await;
        assert_eq!(result, Err(ActionRefusal::NotMember));
        assert_eq!(me.entries().await, before);
    }

    #[tokio::test]
    async fn forget_forgotten_is_not_member() {
        // given: a is forgotten
        let me = setup(&[pid(2)]);
        let a = pid(2);
        me.apply(MemberAction::Forget(a)).await.unwrap();
        assert!(me.ledger().is_forgotten(&a));
        // when / then: it is no member to vote about
        assert_eq!(
            me.apply(MemberAction::Forget(a)).await,
            Err(ActionRefusal::NotMember)
        );
    }

    #[tokio::test]
    async fn left_node_cannot_forget() {
        // given: I have left the network
        let me = setup(&[pid(2), pid(3)]);
        me.apply(MemberAction::Leave).await.unwrap();
        let before = me.entries().await;
        // when / then: neither the vote nor taking it back is allowed, nothing is written
        assert_eq!(
            me.apply(MemberAction::Forget(pid(3))).await,
            Err(ActionRefusal::Left)
        );
        assert_eq!(
            me.apply(MemberAction::Unforget(pid(3))).await,
            Err(ActionRefusal::Left)
        );
        assert_eq!(me.entries().await, before);
    }
}
