//! Handshake tests with a membership list, part two: a pending list holds the handshake back, and the initiator
//! learns of the responder's refusal from the close text. Child module of `handshake_access_tests.rs`: `ids`,
//! `view_with`, `then_refusal` and the other helpers are there.

use super::*;
use crate::transport::net::{BoxFuture, LinkInfo, NetResult, StreamPair};
use tokio::task::JoinHandle;

/// How long a handshake that must stay blocked is given to (wrongly) finish.
const SETTLE: Duration = Duration::from_millis(150);

mod pending_view {
    use super::*;

    #[tokio::test]
    async fn handshake_waits_for_pending_view() {
        // given: B's list has not published yet; both sides start
        let (a, b) = ids();
        let mut fx = setup_with_views(AccessView::open(), AccessView::pending()).await;
        let initiating = spawn_initiate(&fx.ini);
        let responding = spawn_respond(&fx.res);
        tokio::time::sleep(SETTLE).await;
        // then: neither side has finished and B has not taken A in
        assert!(
            !responding.is_finished(),
            "respond did not wait for the list"
        );
        assert!(
            !initiating.is_finished(),
            "initiate finished without an answer"
        );
        assert!(
            !fx.res.ctx.members.is_member(a),
            "B took A in before the list was ready"
        );
        // when: the list is published and contains A
        fx.res
            .ctx
            .members
            .access()
            .publish(verdicts_with(b, &[a], &[]));
        // then: the handshake completes
        let (initiated, responded) = (joined(initiating).await, joined(responding).await);
        assert!(
            initiated.is_ok() && responded.is_ok(),
            "{initiated:?} / {responded:?}"
        );
        fx.res.then_up_for(a);
    }

    #[tokio::test]
    async fn handshake_refuses_once_the_pending_view_denies() {
        // given: B's list has not published yet; both sides start
        let (a, b) = ids();
        let fx = setup_with_views(AccessView::open(), AccessView::pending()).await;
        let initiating = spawn_initiate(&fx.ini);
        let responding = spawn_respond(&fx.res);
        tokio::time::sleep(SETTLE).await;
        assert!(
            !responding.is_finished(),
            "respond did not wait for the list"
        );
        // when: the list is published and does not contain A
        fx.res
            .ctx
            .members
            .access()
            .publish(verdicts_with(b, &[], &[]));
        // then: B refuses with 403
        let (initiated, responded) = (joined(initiating).await, joined(responding).await);
        assert!(
            initiated.is_err() && responded.is_err(),
            "{initiated:?} / {responded:?}"
        );
        fx.res.then_closed_with(403, b"not admitted");
        then_refusal(&fx.res.ctx, a, Refuser::Me, Deny::Unlisted);
    }

    #[tokio::test]
    async fn initiator_waits_for_its_own_pending_view() {
        // given: A's list has not published yet; both sides start
        let (a, b) = ids();
        let fx = setup_with_views(AccessView::pending(), AccessView::open()).await;
        let initiating = spawn_initiate(&fx.ini);
        let responding = spawn_respond(&fx.res);
        tokio::time::sleep(SETTLE).await;
        // then: nothing has been exchanged yet
        assert!(
            !initiating.is_finished(),
            "initiate did not wait for the list"
        );
        assert!(
            !fx.ini.ctx.members.is_member(b),
            "A took B in before the list was ready"
        );
        // when: the list is published and contains B
        fx.ini
            .ctx
            .members
            .access()
            .publish(verdicts_with(a, &[b], &[]));
        // then: the handshake completes
        let (initiated, responded) = (joined(initiating).await, joined(responding).await);
        assert!(
            initiated.is_ok() && responded.is_ok(),
            "{initiated:?} / {responded:?}"
        );
    }
}

mod refusal_reported_by_responder {
    use super::*;

    #[tokio::test]
    async fn initiator_notes_refusal_reported_by_responder() {
        // given: B's list excludes A; the connection A holds reports "code 403: excluded" once closed
        let (a, b) = ids();
        let fx = setup_with_views(AccessView::open(), view_with(b, &[], &[a])).await;
        let conn = reporting(&fx.ini.conn, "code 403: excluded");
        // when: A initiates, B responds
        let (initiated, responded) = fx.when_handshaking_over(&conn).await;
        // then: both fail, and A noted that the other side refused it
        assert!(
            initiated.is_err() && responded.is_err(),
            "{initiated:?} / {responded:?}"
        );
        then_refusal(&fx.ini.ctx, b, Refuser::Them, Deny::Excluded);
    }

    #[tokio::test]
    async fn initiator_notes_every_reason_the_responder_can_give() {
        // given: the four texts a refusing responder can send
        let cases = [
            ("code 403: not admitted", Deny::Unlisted),
            ("code 403: excluded", Deny::Excluded),
            ("code 403: left", Deny::Left),
            ("code 403: refused", Deny::Refused),
        ];
        for (text, deny) in cases {
            let (a, b) = ids();
            let fx = setup_with_views(AccessView::open(), view_with(b, &[], &[a])).await;
            let conn = reporting(&fx.ini.conn, text);
            // when: A initiates against a responder that refuses
            let (initiated, _) = fx.when_handshaking_over(&conn).await;
            // then: A noted the matching reason
            assert!(initiated.is_err(), "initiate with {text:?}: {initiated:?}");
            then_refusal(&fx.ini.ctx, b, Refuser::Them, deny);
            // then: the error carries the same reason as a type
            let typed = initiated
                .unwrap_err()
                .downcast_ref::<crate::members::handshake::RefusedByPeer>()
                .map(|r| r.0);
            assert_eq!(typed, Some(deny), "typed refusal for {text:?}");
        }
    }

    #[tokio::test]
    async fn initiator_notes_nothing_for_other_close_texts() {
        // given: close texts that are not a list refusal (the first is what `mem` reports)
        for text in ["", "code 401: bad secret", "code 403: unknown reason"] {
            let (a, b) = ids();
            let fx = setup_with_views(AccessView::open(), view_with(b, &[], &[a])).await;
            let conn = reporting(&fx.ini.conn, text);
            // when: A initiates against a responder that refuses
            let (initiated, _) = fx.when_handshaking_over(&conn).await;
            // then: it fails, but no refusal is recorded
            assert!(initiated.is_err(), "initiate with {text:?}: {initiated:?}");
            assert!(
                fx.ini.ctx.members.access().refusals().is_empty(),
                "refusal noted for {text:?}"
            );
        }
    }
}

// ---- helpers ----

fn spawn_initiate(side: &Side) -> JoinHandle<anyhow::Result<()>> {
    let (conn, ctx) = (side.conn.clone(), side.ctx.clone());
    crate::node::output::spawn(async move { initiate(&conn, &ctx).await })
}

fn spawn_respond(side: &Side) -> JoinHandle<anyhow::Result<()>> {
    let (conn, ctx) = (side.conn.clone(), side.ctx.clone());
    crate::node::output::spawn(async move { respond(&conn, &ctx).await })
}

async fn joined(task: JoinHandle<anyhow::Result<()>>) -> anyhow::Result<()> {
    must_finish(task).await.expect("handshake task panicked")
}

/// A real connection that reports `text` as its close reason once it is closed — as the hyper transport does after
/// the other side closed it; `mem` reports an empty text.
struct ReportingConn {
    inner: Arc<dyn PeerConn>,
    text: String,
}

fn reporting(inner: &Arc<dyn PeerConn>, text: &str) -> Arc<dyn PeerConn> {
    Arc::new(ReportingConn {
        inner: inner.clone(),
        text: text.to_string(),
    })
}

impl PeerConn for ReportingConn {
    fn peer(&self) -> PeerId {
        self.inner.peer()
    }

    fn stable_id(&self) -> u64 {
        self.inner.stable_id()
    }

    fn open(&self) -> BoxFuture<'_, NetResult<StreamPair>> {
        self.inner.open()
    }

    fn accept(&self) -> BoxFuture<'_, NetResult<StreamPair>> {
        self.inner.accept()
    }

    fn link(&self) -> LinkInfo {
        self.inner.link()
    }

    fn close(&self, code: u32, reason: &[u8]) {
        self.inner.close(code, reason)
    }

    fn closed(&self) -> BoxFuture<'_, ()> {
        self.inner.closed()
    }

    fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    fn close_reason(&self) -> String {
        match self.inner.is_closed() {
            true => self.text.clone(),
            false => String::new(),
        }
    }
}
