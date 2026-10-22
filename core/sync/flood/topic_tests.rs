//! Tests for the topic actor on `mem`: joining a topic, admission, flooding. Breaks and silence are in `topic_down_tests.rs`.

use super::test_support::{
    fast, introduce, is_received, is_up, mem_node, next, probe, quiet, received_count, wait_ups, FloodNode,
    Rig, NETWORK, TOPIC,
};
use super::wire::MAX_BODY_BYTES;
use super::Tuning;
use crate::net::contract::secret;
use crate::net::contract::{pid, must_finish, SEED_A, SEED_B, SEED_C};
use crate::net::id::PeerId;
use crate::net::sync_backend::{Swarm, SyncBackend, SyncCtx};
use crate::net::{NetError, SwarmEvent};
use bytes::Bytes;
use std::sync::Arc;

/// Joins topic `TOPIC`; a join error is a panic.
pub(super) async fn open(node: &FloodNode, peers: &[PeerId]) -> Swarm {
    match must_finish(node.sync.join_swarm(TOPIC, peers)).await {
        Ok(swarm) => swarm,
        Err(e) => panic!("join_swarm did not join the topic: {e}"),
    }
}

/// There is one link: one entry and exactly a pair of tasks (reader, writer); an extra connection would have given a second pair.
pub(super) async fn assert_one_link(node: &FloodNode) {
    let p = probe(node, TOPIC).await;
    assert_eq!((p.links, p.tasks), (1, 2), "not a single link: {p:?}");
}

pub(super) fn hello(tag: &str) -> Bytes {
    Bytes::from(format!("hello from a {tag}"))
}

mod join {
    use super::*;

    #[tokio::test]
    async fn repeat_gives_no_second_up() {
        // given: A and B are peers
        let rig = Rig::new();
        let (a, b) = (rig.node(SEED_A), rig.node(SEED_B));
        introduce(&a, &b);
        let mut sa = open(&a, &[]).await;
        let mut sb = open(&b, &[pid(SEED_A)]).await;
        wait_ups(&mut sa.events, &[pid(SEED_B)]).await;
        wait_ups(&mut sb.events, &[pid(SEED_A)]).await;
        // when: B calls A into the topic again
        must_finish(sb.membership.join(&[pid(SEED_A)]))
            .await
            .expect("repeated join");
        // then: no events for anyone within QUIET, the link works (a single one)
        assert!(quiet(&mut sb.events).await.is_empty(), "B has an event");
        assert!(quiet(&mut sa.events).await.is_empty(), "A has an event");
        must_finish(sa.membership.broadcast(hello("#1")))
            .await
            .expect("broadcast");
        next(&mut sb.events, is_received).await;
        assert_eq!(
            received_count(&quiet(&mut sb.events).await),
            0,
            "the message arrived twice"
        );
        assert_one_link(&a).await;
        assert_one_link(&b).await;
    }

    #[tokio::test]
    async fn self_is_skipped() {
        // given: A alone in the topic
        let rig = Rig::new();
        let a = rig.node(SEED_A);
        let mut sa = open(&a, &[]).await;
        // when: A calls itself into the topic
        let joined = must_finish(sa.membership.join(&[pid(SEED_A)])).await;
        // then: Ok, no events
        assert!(joined.is_ok(), "join of itself returned {joined:?}");
        assert!(quiet(&mut sa.events).await.is_empty(), "an event appeared");
        assert_eq!(probe(&a, TOPIC).await.dials, 0, "a dial to itself was created");
    }

    #[tokio::test]
    async fn before_attach_is_config_error() {
        // given: a sync whose stack was not attached
        let rig = Rig::new();
        let ctx = SyncCtx {
            secret: secret(SEED_A),
            network_seed: NETWORK,
            store_dir: None,
        };
        let delegate: Arc<dyn SyncBackend> = rig.hub.backend(ctx.clone());
        let sync = super::super::FloodSync::new(&ctx, delegate, fast());
        // when: we join a topic with peer B
        let joined = must_finish(sync.join_swarm(TOPIC, &[pid(SEED_B)])).await;
        // then: Config
        assert!(matches!(joined, Err(NetError::Config(_))), "expected Config");
        // and after the stack is attached the same topic opens: the actor of the failed join does not hold it
        let rig2 = Rig::new();
        let stack = rig2.node(SEED_A).stack;
        sync.attach_stack(stack.downgrade());
        let again = must_finish(sync.join_swarm(TOPIC, &[pid(SEED_B)])).await;
        assert!(again.is_ok(), "the repeated join returned {:?}", again.err());
    }

    #[tokio::test]
    async fn crossed_dials_keep_one_link() {
        // given: A and B in the topic, addresses exchanged
        let rig = Rig::new();
        let (a, b) = (rig.node(SEED_A), rig.node(SEED_B));
        introduce(&a, &b);
        let mut sa = open(&a, &[]).await;
        let mut sb = open(&b, &[]).await;
        // when: both call each other at the same time
        let (ja, jb) = tokio::join!(
            sa.membership.join(&[pid(SEED_B)]),
            sb.membership.join(&[pid(SEED_A)])
        );
        ja.and(jb).expect("join");
        // then: one NeighborUp each, then neither Up nor Down within QUIET
        next(&mut sa.events, is_up(pid(SEED_B))).await;
        next(&mut sb.events, is_up(pid(SEED_A))).await;
        assert!(quiet(&mut sa.events).await.is_empty(), "extra event at A");
        assert!(quiet(&mut sb.events).await.is_empty(), "extra event at B");
        // and A's broadcast reaches B exactly once
        must_finish(sa.membership.broadcast(hello("#1")))
            .await
            .expect("broadcast");
        next(&mut sb.events, is_received).await;
        assert_eq!(
            received_count(&quiet(&mut sb.events).await),
            0,
            "the message arrived twice"
        );
        assert_one_link(&a).await;
        assert_one_link(&b).await;
    }
}

mod admit {
    use super::*;

    #[tokio::test]
    async fn foreign_network_never_neighbor() {
        // given: A in the topic; C with a different network seed, addresses known
        let rig = Rig::new();
        let a = rig.node(SEED_A);
        let c = mem_node(&rig.net, &rig.hub, SEED_C, [8; 32], fast());
        let b = rig.node(SEED_B);
        introduce(&a, &c);
        introduce(&a, &b);
        let mut sa = open(&a, &[]).await;
        // when: C calls A into the topic, and in the same time B of its own network calls A
        let mut sc = open(&c, &[pid(SEED_A)]).await;
        let _sb = open(&b, &[pid(SEED_A)]).await;
        // then: A has an Up only from B (control: the dials arrive), C has no events
        let (seen_a, seen_c) = (quiet(&mut sa.events).await, quiet(&mut sc.events).await);
        assert!(seen_a.iter().any(is_up(pid(SEED_B))), "control: no Up(B) {seen_a:?}");
        assert!(!seen_a.iter().any(is_up(pid(SEED_C))), "Up(C) at A {seen_a:?}");
        assert!(seen_c.is_empty(), "events at C {seen_c:?}");
    }
}

mod flood {
    use super::*;

    /// Line A–B–C: A and C do not know each other's addresses. `ttl` is A's.
    async fn line(rig: &Rig, ttl: u8) -> ([FloodNode; 3], Swarm, Swarm, Swarm) {
        let t = Tuning { ttl_hops: ttl, ..fast() };
        let a = mem_node(&rig.net, &rig.hub, SEED_A, NETWORK, t);
        let (b, c) = (rig.node(SEED_B), rig.node(SEED_C));
        introduce(&a, &b);
        introduce(&b, &c);
        let mut sb = open(&b, &[]).await;
        let mut sa = open(&a, &[pid(SEED_B)]).await;
        let mut sc = open(&c, &[pid(SEED_B)]).await;
        wait_ups(&mut sb.events, &[pid(SEED_A), pid(SEED_C)]).await;
        wait_ups(&mut sa.events, &[pid(SEED_B)]).await;
        wait_ups(&mut sc.events, &[pid(SEED_B)]).await;
        ([a, b, c], sa, sb, sc)
    }

    #[tokio::test]
    async fn line_forwards_with_neighbor_as_from() {
        // given: line A–B–C
        let rig = Rig::new();
        let (_nodes, sa, _sb, mut sc) = line(&rig, 6).await;
        // when: A sends a broadcast
        must_finish(sa.membership.broadcast(hello("#1")))
            .await
            .expect("broadcast");
        // then: C received Received { from: B } with the same bytes
        match next(&mut sc.events, is_received).await {
            SwarmEvent::Received { from, msg } => {
                assert_eq!(from, pid(SEED_B), "from whom");
                assert_eq!(msg, hello("#1"), "bytes");
            }
            other => panic!("expected Received, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn ttl_one_stops_at_first_hop() {
        // given: line A–B–C, A has ttl = 1
        let rig = Rig::new();
        let (_nodes, sa, mut sb, mut sc) = line(&rig, 1).await;
        // when: A sends a broadcast
        must_finish(sa.membership.broadcast(hello("#1")))
            .await
            .expect("broadcast");
        // then: B received, C did not within QUIET
        next(&mut sb.events, is_received).await;
        assert_eq!(
            received_count(&quiet(&mut sc.events).await),
            0,
            "C received at ttl = 1"
        );
    }

    #[tokio::test]
    async fn triangle_delivers_once() {
        // given: triangle A, B, C
        let rig = Rig::new();
        let (a, b, c) = (rig.node(SEED_A), rig.node(SEED_B), rig.node(SEED_C));
        introduce(&a, &b);
        introduce(&b, &c);
        introduce(&a, &c);
        let mut sa = open(&a, &[]).await;
        let mut sb = open(&b, &[pid(SEED_A)]).await;
        let mut sc = open(&c, &[pid(SEED_A), pid(SEED_B)]).await;
        wait_ups(&mut sa.events, &[pid(SEED_B), pid(SEED_C)]).await;
        wait_ups(&mut sb.events, &[pid(SEED_A), pid(SEED_C)]).await;
        wait_ups(&mut sc.events, &[pid(SEED_A), pid(SEED_B)]).await;
        // when: A sends a broadcast
        must_finish(sa.membership.broadcast(hello("#1")))
            .await
            .expect("broadcast");
        // then: C has exactly one Received, A has no echo
        next(&mut sc.events, is_received).await;
        assert_eq!(
            received_count(&quiet(&mut sc.events).await),
            0,
            "C received it twice"
        );
        assert_eq!(received_count(&quiet(&mut sa.events).await), 0, "echo at A");
    }

    #[tokio::test]
    async fn oversize_broadcast_is_error() {
        // given: A and B are peers
        let rig = Rig::new();
        let (a, b) = (rig.node(SEED_A), rig.node(SEED_B));
        introduce(&a, &b);
        let sa = open(&a, &[]).await;
        let mut sb = open(&b, &[pid(SEED_A)]).await;
        next(&mut sb.events, is_up(pid(SEED_A))).await;
        // when: A sends a body one byte over the limit
        let sent = must_finish(
            sa.membership
                .broadcast(Bytes::from(vec![1u8; MAX_BODY_BYTES + 1])),
        )
        .await;
        // then: Io(InvalidInput), nothing at B
        let kind = match &sent {
            Err(NetError::Io(e)) => Some(e.kind()),
            _ => None,
        };
        assert_eq!(
            kind,
            Some(std::io::ErrorKind::InvalidInput),
            "returned {sent:?}"
        );
        assert!(quiet(&mut sb.events).await.is_empty(), "B has an event");
    }
}
