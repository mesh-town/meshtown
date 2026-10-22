//! Tests for the edge cases of the peer lifecycle on `mem`: a deferred `NeighborDown`, a peer restart, a slow peer,
//! a connection without `Hello` and `Hello` into an unopened topic.

use super::admit::{accept_handshake, admission_key, dial_handshake, welcome};
use super::test_support::{is_down, is_up, next, quiet, Rig, TOPIC};
use super::test_support::{probe, FloodNode, NETWORK};
use super::topic_tests::{assert_one_link, open};
use super::wire::write_frame;
use super::Tuning;
use crate::transport::net::contract::{must_finish, pid, secret, SEED_A, SEED_C};
use crate::transport::net::id::PeerId;
use crate::transport::net::stream::BoxFuture;
use crate::transport::net::transport::PeerConn;
use crate::transport::net::{NetStack, NetStackBuilder, ServiceHandler, SwarmEvent, SYNC};
use std::sync::Arc;
use std::time::Duration;

/// A raw counterpart: a stack without flooding, which can be called and can be kept mute.
fn raw_stack(rig: &Rig, seed: u8, handler: Option<Arc<dyn ServiceHandler>>) -> NetStack {
    let mut builder =
        NetStackBuilder::new(secret(seed)).transport(rig.net.transport(&secret(seed)));
    if let Some(h) = handler {
        builder = builder.serve(SYNC, h);
    }
    builder.build().expect("raw stack")
}

/// The raw counterpart dialed node `a` but did not open a stream.
async fn raw_conn(rig: &Rig, a: &FloodNode, seed: u8) -> (NetStack, Arc<dyn PeerConn>) {
    let raw = raw_stack(rig, seed, None);
    raw.add_hints(a.stack.invite_addrs()[0].clone());
    let conn = must_finish(raw.connect(pid(SEED_A), SYNC))
        .await
        .expect("connect");
    (raw, conn)
}

/// The accepting side of a node's dial: reads `Hello`, replies `Welcome` after `delay` (silent without `delay`).
struct Answerer {
    me: PeerId,
    delay: Option<Duration>,
}

impl ServiceHandler for Answerer {
    fn handle(&self, conn: Arc<dyn PeerConn>) -> BoxFuture<'static, ()> {
        let (me, delay) = (self.me, self.delay);
        Box::pin(async move {
            let key = admission_key(&NETWORK);
            let mut pair = conn.accept().await.expect("stream");
            let peer = conn.peer();
            let _ = accept_handshake(&mut pair, &key, me, peer, |_| true).await;
            if let Some(d) = delay {
                tokio::time::sleep(d).await;
                let _ = write_frame(&mut pair.send, &welcome(&key, me, peer, &TOPIC)).await;
            }
            tokio::time::sleep(Duration::from_secs(3600)).await;
        })
    }
}

/// A calls the mute C (the dial hangs), and meanwhile C itself joins the topic and breaks the link.
/// Returns A's events after the break within `QUIET`.
async fn lost_while_dialing(
    delay: Option<Duration>,
) -> (FloodNode, super::test_support::Events, Vec<SwarmEvent>) {
    let rig = Rig::new();
    let a = rig.node(SEED_A);
    let handler = Arc::new(Answerer {
        me: pid(SEED_C),
        delay,
    });
    let raw = raw_stack(&rig, SEED_C, Some(handler));
    a.stack.add_hints(raw.invite_addrs()[0].clone());
    raw.add_hints(a.stack.invite_addrs()[0].clone());
    let mut sa = open(&a, &[]).await;
    must_finish(sa.membership.join(&[pid(SEED_C)]))
        .await
        .expect("join");
    let conn = must_finish(raw.connect(pid(SEED_A), SYNC))
        .await
        .expect("connect");
    let mut pair = must_finish(conn.open()).await.expect("stream");
    let key = admission_key(&NETWORK);
    let shake = dial_handshake(&mut pair, &key, pid(SEED_C), pid(SEED_A), &TOPIC);
    must_finish(shake).await.expect("handshake");
    next(&mut sa.events, is_up(pid(SEED_C))).await;
    conn.close(0, b"bye");
    let seen = quiet(&mut sa.events).await;
    std::mem::forget(raw);
    (a, sa.events, seen)
}

mod deferred {
    use super::*;

    #[tokio::test]
    async fn failed_dial_gives_one_down() {
        // given: the link with C ended while our dial to C hangs
        let (a, mut events, seen) = lost_while_dialing(None).await;
        // then: while the dial is in progress there is no Down; after it fails, exactly one
        assert!(seen.is_empty(), "Down before the dial outcome: {seen:?}");
        next(&mut events, is_down(pid(SEED_C))).await;
        assert!(quiet(&mut events).await.is_empty(), "extra event");
        assert_eq!(probe(&a, TOPIC).await.links, 0, "the link remained");
    }

    #[tokio::test]
    async fn successful_dial_gives_no_events() {
        // given: the link with C ended while our dial to C hangs; C will answer the dial after 0.5 s
        let (a, mut events, seen) = lost_while_dialing(Some(Duration::from_millis(500))).await;
        assert!(seen.is_empty(), "Down before the dial outcome: {seen:?}");
        // then: the dial succeeded — neither Down nor Up; there is a single link
        assert!(
            quiet(&mut events).await.is_empty(),
            "there is an event after the successful dial"
        );
        assert_one_link(&a).await;
    }
}

mod restart {
    use super::*;

    /// A raw C's dial into A's topic: the stream and the handshake.
    async fn ring(
        raw: &NetStack,
    ) -> (Arc<dyn PeerConn>, crate::transport::net::stream::StreamPair) {
        let conn = must_finish(raw.connect(pid(SEED_A), SYNC))
            .await
            .expect("connect");
        let mut pair = must_finish(conn.open()).await.expect("stream");
        let key = admission_key(&NETWORK);
        let shake = dial_handshake(&mut pair, &key, pid(SEED_C), pid(SEED_A), &TOPIC);
        must_finish(shake).await.expect("handshake");
        (conn, pair)
    }

    #[tokio::test]
    async fn new_link_of_same_dialer_replaces_old_silently() {
        // given: the raw C is already A's peer (it dialed itself)
        let rig = Rig::new();
        let a = rig.node(SEED_A);
        let mut sa = open(&a, &[]).await;
        let (raw, _) = raw_conn(&rig, &a, SEED_C).await;
        let (old, _old_pair) = ring(&raw).await;
        next(&mut sa.events, is_up(pid(SEED_C))).await;
        // when: the same C dials again (a restart: it has already lost the old link)
        let (new, _new_pair) = ring(&raw).await;
        // then: A has neither Down nor Up, a single link, the old one closed, the new one alive
        let seen = quiet(&mut sa.events).await;
        assert!(seen.is_empty(), "events at A {seen:?}");
        assert_one_link(&a).await;
        assert_ne!(old.stable_id(), new.stable_id(), "the same connection");
        assert!(
            old.is_closed() && !new.is_closed(),
            "the old link was not the one closed"
        );
    }
}

mod edges {
    use super::*;

    #[tokio::test]
    async fn slow_neighbor_closed_by_full_queue() {
        // given: the raw C passed the handshake and does not read; A's write queue is one frame, silence is tolerated for long
        let rig = Rig::new();
        let t = Tuning {
            peer_write_queue_len: 1,
            idle: Duration::from_secs(30),
            keepalive: Duration::from_secs(10),
            ..super::super::test_support::fast()
        };
        let a = rig.node_with(SEED_A, t);
        let mut sa = open(&a, &[]).await;
        let (_raw, conn) = raw_conn(&rig, &a, SEED_C).await;
        let mut pair = must_finish(conn.open()).await.expect("stream");
        let key = admission_key(&NETWORK);
        let shake = dial_handshake(&mut pair, &key, pid(SEED_C), pid(SEED_A), &TOPIC);
        must_finish(shake).await.expect("handshake");
        next(&mut sa.events, is_up(pid(SEED_C))).await;
        // when: A sends many messages in a row
        for i in 0..64 {
            let body = vec![i as u8; 1000];
            must_finish(sa.membership.broadcast(body.into()))
                .await
                .expect("broadcast");
        }
        // then: C is disconnected as slow (idle is far away)
        next(&mut sa.events, is_down(pid(SEED_C))).await;
        drop(pair);
    }

    #[tokio::test]
    async fn connection_without_hello_is_closed() {
        // given: the raw C opened a stream and did not send Hello
        let rig = Rig::new();
        let a = rig.node(SEED_A);
        let mut sa = open(&a, &[]).await;
        let (_raw, conn) = raw_conn(&rig, &a, SEED_C).await;
        let _pair = must_finish(conn.open()).await.expect("stream");
        // then: A closes the connection within the handshake timeout, no peers appeared
        must_finish(conn.closed()).await;
        assert!(quiet(&mut sa.events).await.is_empty(), "an event appeared");
    }

    #[tokio::test]
    async fn hello_to_unopened_topic_is_rejected() {
        // given: A's handshake timeout is 30 s (closing by timeout will not make it), topic [9; 32] is not open
        let rig = Rig::new();
        let t = Tuning {
            handshake: Duration::from_secs(30),
            ..super::super::test_support::fast()
        };
        let a = rig.node_with(SEED_A, t);
        let mut sa = open(&a, &[]).await;
        let (_raw, conn) = raw_conn(&rig, &a, SEED_C).await;
        let mut pair = must_finish(conn.open()).await.expect("stream");
        let key = admission_key(&NETWORK);
        // when: C sends Hello with the right tag into a foreign topic
        let shake = dial_handshake(&mut pair, &key, pid(SEED_C), pid(SEED_A), &[9; 32]);
        let res = must_finish(shake).await;
        // then: the handshake failed, the connection is closed, no events
        assert!(res.is_err(), "the handshake passed");
        must_finish(conn.closed()).await;
        assert!(quiet(&mut sa.events).await.is_empty(), "an event appeared");
    }
}
