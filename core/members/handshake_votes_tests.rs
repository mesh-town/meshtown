//! Handshake tests for a node that was excluded by votes: unlike a node that left on its own, it still judges
//! its neighbours by the ordinary rules, so a listed neighbour gets through and an excluded one is refused.
//! Child module of `handshake_access_tests.rs`: `ids`, `view_with`, `then_refusal` and the other helpers are
//! there.

use super::*;

mod responder {
    use super::*;

    #[tokio::test]
    async fn responder_excluded_by_votes_admits_a_listed_peer() {
        // given: B is excluded by votes, A is in B's list
        let (a, b) = ids();
        let mut fx = setup_with_views(AccessView::open(), view_with(b, &[a], &[b])).await;
        // when: A initiates, B responds
        let (initiated, responded) = fx.when_handshaking().await;
        // then: B does not refuse, the handshake completes
        assert!(responded.is_ok(), "respond: {responded:?}");
        assert!(initiated.is_ok(), "initiate: {initiated:?}");
        assert!(
            fx.res.ctx.members.access().refusals().is_empty(),
            "B refused"
        );
        assert_ne!(
            close_code(&fx.res),
            Some(403),
            "B closed with an access refusal"
        );
        fx.res.then_up_for(a);
    }

    #[tokio::test]
    async fn responder_excluded_by_votes_refuses_an_excluded_peer() {
        // given: B and A are both excluded by votes
        let (a, b) = ids();
        let mut fx = setup_with_views(AccessView::open(), view_with(b, &[], &[a, b])).await;
        // when: A initiates, B responds
        let (initiated, responded) = fx.when_handshaking().await;
        // then: B closes with "excluded", not with "left"
        assert!(
            responded.is_err() && initiated.is_err(),
            "{initiated:?} / {responded:?}"
        );
        fx.res.then_closed_with(403, b"excluded");
        then_refusal(&fx.res.ctx, a, Refuser::Me, Deny::Excluded);
        fx.then_nobody_is_member();
    }
}

mod initiator {
    use super::*;

    #[tokio::test]
    async fn initiator_excluded_by_votes_dials_a_listed_peer() {
        // given: A is excluded by votes, B is in A's list
        let (a, b) = ids();
        let mut fx = setup_with_views(view_with(a, &[b], &[a]), AccessView::open()).await;
        // when: A initiates, B responds
        let (initiated, responded) = fx.when_handshaking().await;
        // then: A does not refuse B on its own side, the handshake completes
        assert!(initiated.is_ok(), "initiate: {initiated:?}");
        assert!(responded.is_ok(), "respond: {responded:?}");
        assert!(
            fx.ini.ctx.members.access().refusals().is_empty(),
            "A refused"
        );
        assert_ne!(
            close_code(&fx.ini),
            Some(403),
            "A closed with an access refusal"
        );
        fx.ini.then_up_for(b);
    }
}

// ---- helpers ----

/// The close code of the side's connection, `None` while it is open.
fn close_code(side: &Side) -> Option<u32> {
    close_info(&side.conn).map(|(code, _)| code)
}
