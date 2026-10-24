//! Tests of the member actions in an administered network: granting and revoking the role, closing
//! the network, and the return of an excluded node.

use super::support::*;
use crate::members::access::actions::{ActionRefusal, MemberAction};
use crate::members::access::record::{admit_key, exclude_key, grant_key, revoke_key, CLOSE_KEY};

mod grant {
    use super::*;

    #[tokio::test]
    async fn a_node_that_is_not_a_member_cannot_be_made_an_administrator() {
        let world = setup(Seat::Root);
        let result = world.apply(MemberAction::Grant(outsider())).await;
        assert_eq!(result, Err(ActionRefusal::NotMember));
    }

    #[tokio::test]
    async fn a_member_becomes_an_administrator() {
        // given: the root and the member x
        let world = setup(Seat::Root);
        // when
        let result = world.apply(MemberAction::Grant(member())).await;
        // then: the record is mine and the member is in the list of administrators
        assert_eq!(result, Ok(true));
        assert!(world.live_ts(&grant_key(&member())).await.is_some());
        assert!(world.ledger().is_admin(&member()));
    }

    #[tokio::test]
    async fn granting_twice_is_unchanged() {
        let world = setup(Seat::Root);
        world.apply(MemberAction::Grant(member())).await.unwrap();
        assert_eq!(world.apply(MemberAction::Grant(member())).await, Ok(false));
    }

    #[tokio::test]
    async fn the_role_revoked_can_be_granted_again() {
        // given: x was made an administrator and then lost the role
        let world = setup(Seat::Root);
        world.apply(MemberAction::Grant(member())).await.unwrap();
        world.apply(MemberAction::Revoke(member())).await.unwrap();
        assert!(!world.ledger().is_admin(&member()));
        // when
        let result = world.apply(MemberAction::Grant(member())).await;
        // then: x is an administrator, and no revoke of mine is newer than the grant
        assert_eq!(result, Ok(true));
        assert!(world.ledger().is_admin(&member()));
        let grant = world.live_ts(&grant_key(&member())).await;
        let revoke = world.live_ts(&revoke_key(&member())).await;
        assert!(
            grant.is_some_and(|g| revoke.is_none_or(|r| g > r)),
            "grant {grant:?} must be later than revoke {revoke:?}"
        );
    }

    #[tokio::test]
    async fn revoke_takes_the_role_away() {
        let world = setup(Seat::Root);
        world.apply(MemberAction::Grant(member())).await.unwrap();
        let result = world.apply(MemberAction::Revoke(member())).await;
        assert_eq!(result, Ok(true));
        assert!(!world.ledger().is_admin(&member()));
    }
}

mod close {
    use super::*;

    #[tokio::test]
    async fn the_administrator_closes_the_network() {
        // given: the root
        let world = setup(Seat::Root);
        // when
        let result = world.apply(MemberAction::Close).await;
        // then: the record is mine and the verdicts say who closed
        assert_eq!(result, Ok(true));
        assert!(world.live_ts(CLOSE_KEY).await.is_some());
        let closed = world.ledger().closed.clone();
        assert_eq!(closed.map(|c| c.by), Some(world.me));
    }

    #[tokio::test]
    async fn every_action_after_it_is_refused() {
        // given: the network is closed
        let world = setup(Seat::Root);
        world.apply(MemberAction::Close).await.unwrap();
        // assumed: also the actions that stay open in a live network (block, leave) are refused
        for act in [
            MemberAction::Exclude(member(), None),
            MemberAction::Unexclude(member()),
            MemberAction::Admit(member()),
            MemberAction::Grant(member()),
            MemberAction::Revoke(member()),
            MemberAction::Close,
            MemberAction::Block(member()),
            MemberAction::Unblock(member()),
            MemberAction::Leave,
        ] {
            // when / then
            let result = world.apply(act.clone()).await;
            assert_eq!(result, Err(ActionRefusal::Closed), "{act:?}");
        }
    }

    #[tokio::test]
    async fn refused_actions_after_it_write_nothing() {
        let world = setup(Seat::Root);
        world.apply(MemberAction::Close).await.unwrap();
        let before = world.shared_entries().await;
        let _ = world.apply(MemberAction::Exclude(member(), None)).await;
        let _ = world.apply(MemberAction::Grant(member())).await;
        assert_eq!(world.shared_entries().await, before);
    }
}

mod return_of_an_excluded_node {
    use super::*;

    #[tokio::test]
    async fn the_administrator_admits_the_node_back_after_the_own_exclusion() {
        // given: x was excluded by me
        let world = setup(Seat::Root);
        world
            .apply(MemberAction::Exclude(member(), None))
            .await
            .unwrap();
        assert!(!world.ledger().is_member(&member()));
        // when
        let result = world.apply(MemberAction::Admit(member())).await;
        // then: x is a member, and the admission is newer than the exclusion
        assert_eq!(result, Ok(true));
        assert!(world.ledger().is_member(&member()));
        let admit = world.live_ts(&admit_key(&member())).await;
        let exclude = world.live_ts(&exclude_key(&member())).await;
        assert!(
            admit.is_some_and(|a| exclude.is_none_or(|e| a > e)),
            "admit {admit:?} must be later than exclude {exclude:?}"
        );
    }

    #[tokio::test]
    async fn the_administrator_admits_the_node_back_after_the_exclusion_by_another_one() {
        // given: x was excluded by the other administrator
        let world = setup(Seat::Root);
        world.given_administrator(other_admin());
        world.given_exclude(5, other_admin(), member());
        world.refresh().await;
        assert!(!world.ledger().is_member(&member()));
        // when
        let result = world.apply(MemberAction::Admit(member())).await;
        // then: x is a member again
        assert_eq!(result, Ok(true));
        assert!(world.ledger().is_member(&member()));
    }
}

mod unexclude {
    use super::*;

    #[tokio::test]
    async fn takes_back_only_the_own_exclusion() {
        // given: both the other administrator and I excluded x
        let world = setup(Seat::Root);
        world.given_administrator(other_admin());
        world.given_exclude(5, other_admin(), member());
        world.given_exclude(6, world.me, member());
        // when
        let result = world.apply(MemberAction::Unexclude(member())).await;
        // then: my record is gone, the other one stays, and x is still excluded
        assert_eq!(result, Ok(true));
        assert_eq!(world.live_ts(&exclude_key(&member())).await, None);
        assert!(world
            .live_of(other_admin(), &exclude_key(&member()))
            .await
            .is_some());
        assert!(!world.ledger().is_member(&member()));
    }

    #[tokio::test]
    async fn changes_nothing_when_only_another_administrator_excluded() {
        // given: only the other administrator excluded x
        let world = setup(Seat::Root);
        world.given_administrator(other_admin());
        world.given_exclude(5, other_admin(), member());
        // when
        let result = world.apply(MemberAction::Unexclude(member())).await;
        // then: nothing of mine to take back, and the exclusion stands
        assert_eq!(result, Ok(false));
        assert!(world
            .live_of(other_admin(), &exclude_key(&member()))
            .await
            .is_some());
        assert!(!world.ledger().is_member(&member()));
    }
}
