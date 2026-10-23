//! More tests of the invite server's owner path for owners the membership list removed: the forgotten
//! owner, the owner removed by the server's own vote, and an owner both excluded and forgotten.
//! The fixture and the helpers are shared with `server_owner_forget_tests.rs`.

use super::forget_tests::*;
use super::*;
use crate::transport::net::facade::test_support::{pid, secret};

mod forgotten_owner {
    use super::*;

    #[tokio::test]
    async fn gets_key_and_admit_and_the_invite_is_burned() {
        // given: the server forgot the owner (two members, one vote forgets) long ago
        let node = setup_node(view_of(&[owner_id()], vec![forget(owner_id(), 100)])).await;
        node.then_view_says(owner_id(), Err(Deny::Forgotten));
        // when: the owner presents the invite
        let reply = node.when_owner_asks(&secret(7)).await;
        // then: the key, the owner's admit record and the used mark
        assert_key(reply);
        assert!(node.admit_of(owner_id()).await.is_some(), "no admit record");
        assert_eq!(
            node.count("invite-used/").await,
            1,
            "the invite was not burned"
        );
    }

    #[tokio::test]
    async fn admit_is_stamped_later_than_the_forget_vote_in_the_future() {
        // given: the forget vote carries a time far ahead of the server's clock
        let mark = now() + 10_000;
        let node = setup_node(view_of(&[owner_id()], vec![forget(owner_id(), mark)])).await;
        // when
        assert_key(node.when_owner_asks(&secret(7)).await);
        // then: the admit is one second after the vote, not at the clock
        let admit = node.admit_of(owner_id()).await.expect("no admit record");
        assert_eq!(
            admit.made_unix,
            mark + 1,
            "the admit is not later than the vote"
        );
    }

    #[tokio::test]
    async fn admit_is_stamped_with_the_clock_when_the_forget_vote_is_old() {
        // given: an old forget vote
        let node = setup_node(view_of(&[owner_id()], vec![forget(owner_id(), 100)])).await;
        let before = now();
        // when
        assert_key(node.when_owner_asks(&secret(7)).await);
        // then: the stamp is the clock reading of the grant
        let admit = node.admit_of(owner_id()).await.expect("no admit record");
        assert!(
            admit.made_unix >= before && admit.made_unix <= now(),
            "made_unix {} is off the clock",
            admit.made_unix
        );
    }

    #[tokio::test]
    async fn own_older_admit_is_overwritten_with_a_later_one() {
        // given: the server's old admit record of the owner is in the list, then the owner was forgotten
        let mark = now() + 500;
        let node = setup_node(view_of(&[owner_id()], vec![forget(owner_id(), mark)])).await;
        node.given_own_admit(owner_id(), 5).await;
        // when
        assert_key(node.when_owner_asks(&secret(7)).await);
        // then: the record under the same key is the new one
        let admit = node.admit_of(owner_id()).await.expect("no admit record");
        assert!(
            admit.made_unix > mark,
            "the old admit (5) was not overwritten: {}",
            admit.made_unix
        );
        assert_eq!(
            node.count("acl/admit/").await,
            1,
            "a second record appeared"
        );
    }

    #[tokio::test]
    async fn without_a_forget_vote_the_admit_is_stamped_with_the_clock() {
        // given: an owner nobody voted about (unknown to the list)
        let node = setup_node(view_of(&[], Vec::new())).await;
        let before = now();
        // when
        assert_key(node.when_owner_asks(&secret(7)).await);
        // then
        let admit = node.admit_of(owner_id()).await.expect("no admit record");
        assert!(
            admit.made_unix >= before && admit.made_unix <= now(),
            "made_unix {} is off the clock",
            admit.made_unix
        );
    }
}

mod owner_removed_by_my_vote {
    use super::*;

    #[tokio::test]
    async fn waiting_round_owner_invite_spends_round() {
        // given: four members, so one vote is not enough; my forget vote about the owner waits for a second
        let members = [pid(3), pid(4), owner_id()];
        let input = input_of(&members, vec![forget(owner_id(), 100)]);
        let node = setup_node(access_of(&input)).await;
        node.then_view_says(owner_id(), Err(Deny::RemovedByMe));
        // when: the owner presents the invite
        let reply = node.when_owner_asks(&secret(7)).await;
        // then: the key and an admit later than my vote; counted again, the round is gone and the owner is a member
        assert_key(reply);
        let admit = node.admit_of(owner_id()).await.expect("no admit record");
        let after = verdicts_of(&with_admit(input, owner_id(), admit.made_unix));
        assert!(
            after.ledger.forgets.is_empty(),
            "the round is still open: {:?}",
            after.ledger.forgets
        );
        assert!(after.my_forgets().is_empty(), "my vote still stands");
        assert!(after.ledger.is_member(&owner_id()));
    }

    #[tokio::test]
    async fn others_waiting_round_does_not_stop_the_invite() {
        // given: someone else's vote about the owner waits, mine does not exist
        let members = [pid(3), pid(4), owner_id()];
        let input = input_of(&members, vec![forget_by(pid(3), owner_id(), 100)]);
        let node = setup_node(access_of(&input)).await;
        node.then_view_says(owner_id(), Ok(()));
        // when / then
        assert_key(node.when_owner_asks(&secret(7)).await);
    }
}

mod excluded_and_forgotten_owner {
    use super::*;

    #[tokio::test]
    async fn gets_excluded_and_invite_unspent() {
        // given: the ledger holds the owner as excluded and as forgotten at once
        let node = setup_node(access_with(excluded_and_forgotten(owner_id()))).await;
        node.then_view_says(owner_id(), Err(Deny::Excluded));
        // when
        let reply = node.when_owner_asks(&secret(7)).await;
        // then: Excluded wins, nothing is written, the invite works for another key
        assert_refused(reply, RedeemCode::Excluded);
        node.then_invite_unspent().await;
    }
}

mod forgotten_server {
    use super::*;

    #[tokio::test]
    async fn forgotten_server_refuses_and_keeps_invite_unspent() {
        // given: two other members forgot the server itself
        let members = [me(), pid(3), pid(4), owner_id()];
        let votes = vec![forget_by(pid(3), me(), 100), forget_by(pid(4), me(), 100)];
        let node = setup_node(access_of(&input_of(&members, votes))).await;
        // when: a newcomer presents the invite
        let reply = node.when_owner_asks(&secret(7)).await;
        // then: a retryable refusal, nothing is written, the invite is still good
        assert_refused(reply, RedeemCode::NotReady);
        node.then_invite_unspent().await;
    }
}
