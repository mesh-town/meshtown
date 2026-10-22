//! Tests for the topic actor on `mem`: a break, silence, stopping the pump, a closed stack.

use super::admit::{admission_key, dial_handshake};
use super::test_support::NETWORK;
use super::Tuning;
use super::test_support::{is_down, is_up, next, quiet, wait_ups, Rig, TOPIC};
use super::topic_tests::open;
use crate::net::contract::secret;
use crate::net::contract::{pid, must_finish, SEED_A, SEED_B, SEED_C};
use crate::net::{NetError, NetStackBuilder, SwarmEvent, SYNC};
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;

/// Silence and `Ping` do not have time to fire: `NeighborDown` within `STEP` is possible only from the link closing.
fn patient() -> Tuning {
    Tuning {
        idle: Duration::from_secs(30),
        keepalive: Duration::from_secs(10),
        ..super::test_support::fast()
    }
}

mod down {
    use super::*;

    #[tokio::test]
    async fn cut_brings_neighbor_down() {
        // given: A and B are peers
        let rig = Rig::new();
        let (a, b) = (rig.node_with(SEED_A, patient()), rig.node_with(SEED_B, patient()));
        super::super::test_support::introduce(&a, &b);
        let mut sa = open(&a, &[]).await;
        let mut sb = open(&b, &[pid(SEED_A)]).await;
        wait_ups(&mut sa.events, &[pid(SEED_B)]).await;
        wait_ups(&mut sb.events, &[pid(SEED_A)]).await;
        // when: the path between them is cut
        rig.net.cut(pid(SEED_A), pid(SEED_B));
        // then: NeighborDown at both
        next(&mut sa.events, is_down(pid(SEED_B))).await;
        next(&mut sb.events, is_down(pid(SEED_A))).await;
    }

    #[tokio::test]
    async fn silent_neighbor_dropped_after_idle() {
        // given: a raw counterpart C passed the SYNC handshake with A and is silent
        let rig = Rig::new();
        let a = rig.node(SEED_A);
        let raw = NetStackBuilder::new(secret(SEED_C))
            .transport(rig.net.transport(&secret(SEED_C)))
            .build()
            .expect("raw stack");
        raw.add_hints(a.stack.invite_addrs()[0].clone());
        let mut sa = open(&a, &[]).await;
        let conn = must_finish(raw.connect(pid(SEED_A), SYNC)).await.expect("connect");
        let mut pair = must_finish(conn.open()).await.expect("stream");
        let key = admission_key(&NETWORK);
        must_finish(dial_handshake(
            &mut pair,
            &key,
            pid(SEED_C),
            pid(SEED_A),
            &TOPIC,
        ))
        .await
        .expect("handshake");
        next(&mut sa.events, is_up(pid(SEED_C))).await;
        // when: C is silent for longer than idle
        // then: NeighborDown(C) within idle + STEP
        next(&mut sa.events, is_down(pid(SEED_C))).await;
        drop(pair);
    }

    #[tokio::test]
    async fn pump_abort_closes_links() {
        // given: A and B are peers
        let rig = Rig::new();
        let (a, b) = (rig.node(SEED_A), rig.node_with(SEED_B, patient()));
        super::super::test_support::introduce(&a, &b);
        let sa = open(&a, &[]).await;
        let mut sb = open(&b, &[pid(SEED_A)]).await;
        wait_ups(&mut sb.events, &[pid(SEED_A)]).await;
        let mut events_a = sa.membership.subscribe();
        // when: A's pump is stopped and A's topic handle is destroyed
        sa.pump.expect("pump").abort();
        drop(sa.membership);
        // then: B gets NeighborDown(A); A's receiver is Closed
        next(&mut sb.events, is_down(pid(SEED_A))).await;
        let end = must_finish(async {
            loop {
                match events_a.recv().await {
                    Ok(_) => continue,
                    Err(e) => return e,
                }
            }
        })
        .await;
        assert!(
            matches!(end, RecvError::Closed),
            "expected Closed, got {end:?}"
        );
    }

    #[tokio::test]
    async fn closed_stack_ends_topic() {
        // given: A in the topic, A's stack is closed
        let rig = Rig::new();
        let a = rig.node(SEED_A);
        let mut sa = open(&a, &[]).await;
        a.stack.close().await;
        // when: A calls B
        must_finish(sa.membership.join(&[pid(SEED_B)]))
            .await
            .expect("join");
        // then: a Closed event
        let got = next(&mut sa.events, |e| matches!(e, SwarmEvent::Closed(_))).await;
        assert!(matches!(got, SwarmEvent::Closed(_)));
        assert!(quiet(&mut sa.events)
            .await
            .iter()
            .all(|e| !matches!(e, SwarmEvent::NeighborUp(_))));
        // the actor ended: the next join of the topic is Closed
        let again = must_finish(sa.membership.join(&[pid(SEED_B)])).await;
        assert!(matches!(again, Err(NetError::Closed)), "expected Closed, got {again:?}");
    }
}
