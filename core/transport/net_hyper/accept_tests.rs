//! `AcceptCtx`: tracking of live sessions (`track`) and closing them (`close_sessions`).
//! Connections run over `tokio::io::duplex`; the dial "network" is never called.

use super::*;
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::id::NodeSecret;
use crate::transport::net::stream::BoxFuture;
use crate::transport::net_hyper::conn::establish_dialed_session;
use tokio::io::duplex;

const SESSION: u64 = 0xABCD_0002;

fn peer(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

/// A dialer that must never happen in these tests.
struct NoDial;

impl StreamDialer for NoDial {
    fn dial(&self, _to: PeerId) -> BoxFuture<'_, NetResult<Box<dyn Duplex>>> {
        Box::pin(async { Err(NetError::NoRoute) })
    }
}

fn ctx() -> AcceptCtx {
    let (accepted_tx, _rx) = mpsc::channel::<Accepted>(1);
    AcceptCtx::new(Arc::new(NoDial), accepted_tx, Arc::new(PeerGate::open()))
}

/// A pair of sessions A (dialed) and B (accepted) with number `session`; each has its own table.
async fn pair(session: u64) -> (Arc<HyperConn>, Arc<HyperConn>) {
    let (table_a, table_b) = (Arc::new(Sessions::new()), Arc::new(Sessions::new()));
    let (ca, mut cb) = duplex(8192);
    let accept = async {
        read_header(&mut cb).await.unwrap();
        establish_accepted_session(peer(1), session, Box::new(cb), Arc::new(NoDial), table_b).await
    };
    let dial = establish_dialed_session(
        peer(2),
        session,
        b"svc",
        Box::new(ca),
        Arc::new(NoDial),
        table_a,
    );
    let (a, b) = tokio::join!(dial, accept);
    (a.unwrap(), b.unwrap())
}

fn live_len(ctx: &AcceptCtx) -> usize {
    ctx.live_conns.lock().unwrap().len()
}

#[tokio::test]
async fn track_drops_entries_of_dead_connections() {
    // given: one session is tracked and dropped; when: a second one is tracked; then: only the second is in the list
    let ctx = ctx();
    let (a, b) = pair(SESSION).await;
    ctx.track(&a);
    drop(a);
    ctx.track(&b);
    assert_eq!(live_len(&ctx), 1);
    assert!(ctx.live_conns.lock().unwrap()[0].upgrade().is_some());
}

#[tokio::test]
async fn close_sessions_closes_live_connections_and_counts_them() {
    // given: two live sessions; when: closed with a code; then: both are closed, count = 2
    let ctx = ctx();
    let (a, b) = pair(SESSION).await;
    ctx.track(&a);
    ctx.track(&b);
    assert_eq!(ctx.close_sessions(7, b"stop"), 2);
    assert!(a.is_closed());
    assert!(b.is_closed());
}

#[tokio::test]
async fn close_sessions_skips_dead_connections() {
    // given: a live and a dropped session; when: closed; then: one is counted
    let ctx = ctx();
    let (a, b) = pair(SESSION).await;
    ctx.track(&a);
    ctx.track(&b);
    drop(a);
    assert_eq!(ctx.close_sessions(7, b"stop"), 1);
    assert!(b.is_closed());
}

#[tokio::test]
async fn close_sessions_empties_live_list_and_session_table() {
    // given: a tracked session and a table entry; when: closed; then: the list and the table are empty
    let ctx = ctx();
    let (a, _b) = pair(SESSION).await;
    ctx.track(&a);
    let key = SessionKey {
        peer: peer(1),
        session: 9,
        numbered_by_us: true,
    };
    let (tx, mut rx) = mpsc::channel(1);
    ctx.sessions.insert(key, tx);
    ctx.close_sessions(7, b"stop");
    assert_eq!(live_len(&ctx), 0);
    assert_eq!(
        ctx.sessions.reserve(key).err(),
        Some(crate::transport::net_hyper::wire::REFUSE_NO_SESSION)
    );
    assert!(rx.recv().await.is_none());
}
