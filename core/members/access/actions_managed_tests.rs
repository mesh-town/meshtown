//! Tests of the member actions in an administered network (`actions.rs`): who may do what, the refusals,
//! and the records the administrator actions write. Each test runs against a real in-memory shared
//! replica with the verdicts recomputed by `Access`.
//! Nodes: `me` (seed 1) and the root (seed 2, or `me` itself), the member `x` (3), the other administrator
//! `other_admin` (4) and the `outsider` (9), who is never admitted.

use super::support::*;
use crate::members::access::actions::{ActionRefusal, MemberAction};
use crate::members::access::record::{exclude_key, parse_entry, Parsed, Via};

mod member_without_power {
    use super::*;

    #[tokio::test]
    async fn exclude_is_not_admin() {
        let world = setup(Seat::Member);
        let result = world.apply(MemberAction::Exclude(member(), None)).await;
        assert_eq!(result, Err(ActionRefusal::NotAdmin));
    }

    #[tokio::test]
    async fn admit_is_not_admin() {
        let world = setup(Seat::Member);
        let result = world.apply(MemberAction::Admit(member())).await;
        assert_eq!(result, Err(ActionRefusal::NotAdmin));
    }

    #[tokio::test]
    async fn grant_is_not_admin() {
        let world = setup(Seat::Member);
        let result = world.apply(MemberAction::Grant(member())).await;
        assert_eq!(result, Err(ActionRefusal::NotAdmin));
    }

    #[tokio::test]
    async fn revoke_is_not_admin() {
        let world = setup(Seat::Member);
        let result = world.apply(MemberAction::Revoke(member())).await;
        assert_eq!(result, Err(ActionRefusal::NotAdmin));
    }

    #[tokio::test]
    async fn close_is_not_admin() {
        let world = setup(Seat::Member);
        let result = world.apply(MemberAction::Close).await;
        assert_eq!(result, Err(ActionRefusal::NotAdmin));
    }

    #[tokio::test]
    async fn a_refused_action_writes_nothing() {
        // given: a plain member
        let world = setup(Seat::Member);
        let before = world.shared_entries().await;
        // when: it tries every action of power
        for act in [
            MemberAction::Exclude(member(), None),
            MemberAction::Admit(member()),
            MemberAction::Grant(member()),
            MemberAction::Revoke(member()),
            MemberAction::Close,
        ] {
            let _ = world.apply(act).await;
        }
        // then: the replica is as it was
        assert_eq!(world.shared_entries().await, before);
    }

    #[tokio::test]
    async fn blocking_a_node_stays_open() {
        let world = setup(Seat::Member);
        assert_eq!(world.apply(MemberAction::Block(member())).await, Ok(true));
    }

    #[tokio::test]
    async fn leaving_stays_open() {
        let world = setup(Seat::Member);
        assert_eq!(world.apply(MemberAction::Leave).await, Ok(true));
    }
}

mod last_administrator {
    use super::*;

    #[tokio::test]
    async fn the_only_administrator_may_not_leave() {
        // given: the root alone is the administrator
        let world = setup(Seat::Root);
        // when / then
        let result = world.apply(MemberAction::Leave).await;
        assert_eq!(result, Err(ActionRefusal::LastAdmin));
    }

    #[tokio::test]
    async fn the_only_administrator_may_not_revoke_the_own_role() {
        // given: I am an administrator granted by the root, and the root has left
        let world = setup(Seat::Admin);
        world.given_leave(3, world.root);
        // when / then
        let result = world.apply(MemberAction::Revoke(world.me)).await;
        assert_eq!(result, Err(ActionRefusal::LastAdmin));
    }

    #[tokio::test]
    async fn a_refused_leave_writes_nothing() {
        let world = setup(Seat::Root);
        let before = world.shared_entries().await;
        let _ = world.apply(MemberAction::Leave).await;
        assert_eq!(world.shared_entries().await, before);
    }

    #[tokio::test]
    async fn an_administrator_may_leave_while_another_one_stays() {
        // given: I am an administrator besides the root
        let world = setup(Seat::Admin);
        // when / then
        assert_eq!(world.apply(MemberAction::Leave).await, Ok(true));
    }

    #[tokio::test]
    async fn an_administrator_may_revoke_the_own_role_while_another_one_stays() {
        // given: I am an administrator besides the root
        let world = setup(Seat::Admin);
        // when
        let result = world.apply(MemberAction::Revoke(world.me)).await;
        // then: the role is gone
        assert_eq!(result, Ok(true));
        assert!(!world.ledger().is_admin(&world.me));
    }
}

mod the_root {
    use super::*;

    #[tokio::test]
    async fn the_role_of_the_root_cannot_be_revoked() {
        // given: I am an administrator, the root is another one
        let world = setup(Seat::Admin);
        // when / then
        let result = world.apply(MemberAction::Revoke(world.root)).await;
        assert_eq!(result, Err(ActionRefusal::NotAdmin));
    }
}

mod vote_record {
    use super::*;

    #[tokio::test]
    async fn managed_exclude_writes_no_moment_mark() {
        // given: an administered network, I am the administrator
        let world = setup(Seat::Root);
        // when: I exclude a member
        world
            .apply(MemberAction::Exclude(member(), None))
            .await
            .unwrap();
        // then: my record is a plain exclusion, the via is the old one
        let entry = world.live_of(world.me, &exclude_key(&member())).await;
        match parse_entry(&entry.expect("the exclusion must be stored")) {
            Parsed::Record { value, .. } => assert_eq!(value.via, Via::None),
            other => panic!("not a record: {other:?}"),
        }
    }
}
