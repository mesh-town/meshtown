//! Handshake tests with a membership list behind each side: the responder and the initiator refuse a node the list
//! denies (close code 403 and the reason on the wire), a pending list holds the handshake back, and the initiator
//! learns of the responder's refusal from the close text. Child module of `handshake_peer_tests.rs`: `Fixture`,
//! `setup_with_views` and the other helpers are there.

use super::*;
use crate::members::access::tally::{Exclusion, How, Ledger};
use crate::members::access::view::{AccessView, Deny, Side as Refuser, Verdicts};
use std::collections::{BTreeMap, BTreeSet};

mod responder {
    use super::*;

    #[tokio::test]
    async fn responder_refuses_unlisted_initiator_with_403() {
        // given: B's list does not contain A
        let (a, b) = ids();
        let mut fx = setup_with_views(AccessView::open(), view_with(b, &[], &[])).await;
        // when: A initiates, B responds
        let (initiated, responded) = fx.when_handshaking().await;
        // then: B refuses and closes the connection with 403 "not admitted"; A fails as well
        assert!(responded.is_err(), "respond: {responded:?}");
        assert!(initiated.is_err(), "initiate: {initiated:?}");
        fx.res.then_closed_with(403, b"not admitted");
        // then: B noted its own refusal, nobody became a member
        then_refusal(&fx.res.ctx, a, Refuser::Me, Deny::Unlisted);
        fx.then_nobody_is_member();
    }

    #[tokio::test]
    async fn responder_refuses_excluded_initiator_with_excluded_reason() {
        // given: B's list excludes A
        let (a, b) = ids();
        let mut fx = setup_with_views(AccessView::open(), view_with(b, &[], &[a])).await;
        // when: A initiates, B responds
        let (initiated, responded) = fx.when_handshaking().await;
        // then: both fail, the close text says "excluded"
        assert!(
            responded.is_err() && initiated.is_err(),
            "{initiated:?} / {responded:?}"
        );
        fx.res.then_closed_with(403, b"excluded");
        then_refusal(&fx.res.ctx, a, Refuser::Me, Deny::Excluded);
        fx.then_nobody_is_member();
    }

    #[tokio::test]
    async fn responder_that_voted_against_initiator_says_only_refused() {
        // given: B voted against A, A is still listed
        let (a, b) = ids();
        let voted = Verdicts {
            my_votes: set(&[a]),
            ..verdicts_with(b, &[a], &[])
        };
        let fx = setup_with_views(AccessView::open(), view_of(voted)).await;
        // when: A initiates, B responds
        let (initiated, responded) = fx.when_handshaking().await;
        // then: the wire reason does not reveal the vote, B's own record does
        assert!(
            responded.is_err() && initiated.is_err(),
            "{initiated:?} / {responded:?}"
        );
        fx.res.then_closed_with(403, b"refused");
        then_refusal(&fx.res.ctx, a, Refuser::Me, Deny::VotedByMe);
    }

    #[tokio::test]
    async fn responder_that_left_refuses_everyone_with_left() {
        // given: B left the network itself; A is listed
        let (a, b) = ids();
        let fx = setup_with_views(AccessView::open(), view_after_leaving(b, &[a])).await;
        // when: A initiates, B responds
        let (initiated, responded) = fx.when_handshaking().await;
        // then: B closes with "left"
        assert!(
            responded.is_err() && initiated.is_err(),
            "{initiated:?} / {responded:?}"
        );
        fx.res.then_closed_with(403, b"left");
        then_refusal(&fx.res.ctx, a, Refuser::Me, Deny::Left);
    }

    #[tokio::test]
    async fn responder_refuses_before_the_initiator_sends_anything() {
        // given: B's list does not contain A, and A stays silent
        let (_, b) = ids();
        let fx = setup_with_views(AccessView::open(), view_with(b, &[], &[])).await;
        // when: only B responds
        let responded = fx.when_responding_alone(Duration::from_secs(2)).await;
        // then: it refuses at once, without waiting for the initiator's stream
        assert!(responded.is_err(), "respond: {responded:?}");
        fx.res.then_closed_with(403, b"not admitted");
    }
}

mod initiator {
    use super::*;

    #[tokio::test]
    async fn initiator_refuses_excluded_responder() {
        // given: A's list excludes B
        let (a, b) = ids();
        let mut fx = setup_with_views(view_with(a, &[], &[b]), AccessView::open()).await;
        // when: A initiates, B responds
        let (initiated, responded) = fx.when_handshaking().await;
        // then: A refuses and closes the connection with 403 "excluded"; B fails as well
        assert!(initiated.is_err(), "initiate: {initiated:?}");
        assert!(responded.is_err(), "respond: {responded:?}");
        fx.ini.then_closed_with(403, b"excluded");
        // then: A noted its own refusal, nobody became a member
        then_refusal(&fx.ini.ctx, b, Refuser::Me, Deny::Excluded);
        fx.then_nobody_is_member();
        // then: the error says it is our own refusal, not the peer's
        then_own_refusal(&initiated, Deny::Excluded);
    }

    #[tokio::test]
    async fn initiator_refuses_unlisted_responder_with_not_admitted() {
        // given: A's list does not contain B
        let (a, b) = ids();
        let mut fx = setup_with_views(view_with(a, &[], &[]), AccessView::open()).await;
        // when: A initiates, B responds
        let (initiated, responded) = fx.when_handshaking().await;
        // then: A refuses with "not admitted"
        assert!(
            initiated.is_err() && responded.is_err(),
            "{initiated:?} / {responded:?}"
        );
        fx.ini.then_closed_with(403, b"not admitted");
        then_refusal(&fx.ini.ctx, b, Refuser::Me, Deny::Unlisted);
        fx.then_nobody_is_member();
        // then: the error says it is our own refusal, not the peer's
        then_own_refusal(&initiated, Deny::Unlisted);
    }

    #[tokio::test]
    async fn initiator_that_left_refuses_everyone_with_left() {
        // given: A left the network itself; B is listed
        let (a, b) = ids();
        let fx = setup_with_views(view_after_leaving(a, &[b]), AccessView::open()).await;
        // when: A initiates, B responds
        let (initiated, responded) = fx.when_handshaking().await;
        // then: A closes with "left"
        assert!(
            initiated.is_err() && responded.is_err(),
            "{initiated:?} / {responded:?}"
        );
        fx.ini.then_closed_with(403, b"left");
        then_refusal(&fx.ini.ctx, b, Refuser::Me, Deny::Left);
        // then: the error says it is our own refusal, not the peer's
        then_own_refusal(&initiated, Deny::Left);
    }
}

mod listed {
    use super::*;

    #[tokio::test]
    async fn listed_peers_complete_as_before() {
        // given: each side's list contains the other
        let (a, b) = ids();
        let mut fx = setup_with_views(view_with(a, &[b], &[]), view_with(b, &[a], &[])).await;
        // when: A initiates, B responds
        let (initiated, responded) = fx.when_handshaking().await;
        // then: both succeed, both count the peer as a member, both got Up
        assert!(initiated.is_ok(), "initiate: {initiated:?}");
        assert!(responded.is_ok(), "respond: {responded:?}");
        assert!(fx.ini.ctx.members.is_member(b), "A did not add B");
        assert!(fx.res.ctx.members.is_member(a), "B did not add A");
        fx.ini.then_up_for(b);
        fx.res.then_up_for(a);
        // then: no refusal was noted anywhere
        assert!(
            fx.ini.ctx.members.access().refusals().is_empty(),
            "A noted a refusal"
        );
        assert!(
            fx.res.ctx.members.access().refusals().is_empty(),
            "B noted a refusal"
        );
    }
}

#[path = "handshake_access_flow_tests.rs"]
mod flow;

#[path = "handshake_votes_tests.rs"]
mod votes;

// ---- helpers ----

/// The ids of the initiator A and the responder B.
fn ids() -> (PeerId, PeerId) {
    (pid(SEED_A), pid(SEED_B))
}

fn set(ids: &[PeerId]) -> BTreeSet<PeerId> {
    ids.iter().copied().collect()
}

/// Verdicts of `me`: `listed` are admitted, `excluded` are admitted and then excluded by votes; no votes, no blocks.
fn verdicts_with(me: PeerId, listed: &[PeerId], excluded: &[PeerId]) -> Verdicts {
    let by_votes = || Exclusion {
        how: How::Votes,
        at_micros: 10,
        voters: Vec::new(),
    };
    let ledger = Ledger {
        admitted: [&[me][..], listed, excluded].concat().into_iter().collect(),
        excluded: excluded.iter().map(|p| (*p, by_votes())).collect(),
        pending: BTreeMap::new(),
        need: 1,
        admissions: BTreeMap::new(),
        forgets: Default::default(),
        admit_unix: Default::default(),
        forget_marks: Default::default(),
        forget_need: 1,
        admins: Default::default(),
        closed: None,
    };
    Verdicts {
        me,
        ledger: Arc::new(ledger),
        my_votes: BTreeSet::new(),
        blocks: BTreeSet::new(),
        reasons: Default::default(),
    }
}

/// A view that already holds `v`.
fn view_of(v: Verdicts) -> AccessView {
    let view = AccessView::open();
    view.publish(v);
    view
}

fn view_with(me: PeerId, listed: &[PeerId], excluded: &[PeerId]) -> AccessView {
    view_of(verdicts_with(me, listed, excluded))
}

/// A view of `me` that left the network on its own (`How::Left`); `listed` are admitted.
fn view_after_leaving(me: PeerId, listed: &[PeerId]) -> AccessView {
    let mut v = verdicts_with(me, listed, &[me]);
    let ledger = Arc::make_mut(&mut v.ledger);
    ledger.excluded.get_mut(&me).expect("me is excluded").how = How::Left;
    view_of(v)
}

/// The handshake error is our own refusal with this reason, and does not claim that the peer refused.
fn then_own_refusal(result: &anyhow::Result<impl std::fmt::Debug>, deny: Deny) {
    let err = result.as_ref().unwrap_err();
    assert_eq!(
        err.downcast_ref::<crate::members::handshake::RefusedByUs>()
            .map(|r| r.0),
        Some(deny),
        "error chain: {err:#}"
    );
    assert!(
        err.downcast_ref::<crate::members::handshake::RefusedByPeer>()
            .is_none(),
        "the error must not say the peer refused: {err:#}"
    );
}

/// The one refusal `ctx` has noted: about `peer`, by `by`, for `deny`.
fn then_refusal(ctx: &MemberContext, peer: PeerId, by: Refuser, deny: Deny) {
    let list = ctx.members.access().refusals();
    assert_eq!(list.len(), 1, "number of noted refusals");
    let (noted, refusal) = &list[0];
    assert_eq!(*noted, peer, "refusal names another node");
    assert_eq!(refusal.side, by, "who refused");
    assert_eq!(refusal.deny, deny, "why");
}

impl Fixture {
    /// Nobody became a member and neither side got an Up event.
    fn then_nobody_is_member(&mut self) {
        let (a, b) = (self.ini.ctx.own_id, self.res.ctx.own_id);
        assert!(!self.ini.ctx.members.is_member(b), "A added B");
        assert!(!self.res.ctx.members.is_member(a), "B added A");
        assert!(self.ini.events.try_recv().is_err(), "A has an event");
        assert!(self.res.events.try_recv().is_err(), "B has an event");
    }

    /// A initiates over `ini_conn` (a wrapper of A's connection), B responds on its own — simultaneously.
    async fn when_handshaking_over(
        &self,
        ini_conn: &Arc<dyn PeerConn>,
    ) -> (anyhow::Result<()>, anyhow::Result<()>) {
        tokio::join!(
            must_finish(initiate(ini_conn, &self.ini.ctx)),
            must_finish(respond(&self.res.conn, &self.res.ctx)),
        )
    }
}

impl Side {
    /// The connection was closed locally with `code` and `reason`.
    fn then_closed_with(&self, code: u32, reason: &'static [u8]) {
        let want = Some((code, Bytes::from_static(reason)));
        assert_eq!(close_info(&self.conn), want, "close code and reason");
    }
}
