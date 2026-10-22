//! Membership contract suite for a broadcast topic: the `membership_contract!` macro and cases (`cases`).
//! The harness is the same as for the replica: `async fn() -> SyncPair` (conventions are at `SyncPair`).
//! Plugging in an adapter: `membership_contract!(async_fn_without_arguments_returning_SyncPair);`.
//! `close_ends_topic` is not included: it depends on the transport and lives in the adapter's tests.

macro_rules! membership_contract {
    ($harness:path) => {
        mod membership {
            use super::*;
            use $crate::transport::net::contract::membership::cases;

            mod two_nodes {
                use super::*;

                #[tokio::test]
                async fn two_nodes_see_each_other_up() {
                    cases::two_nodes_see_each_other_up($harness().await).await
                }

                #[tokio::test]
                async fn broadcast_reaches_neighbor() {
                    cases::broadcast_reaches_neighbor($harness().await).await
                }

                #[tokio::test]
                async fn second_subscriber_gets_events() {
                    cases::second_subscriber_gets_events($harness().await).await
                }
            }

            mod lone_node {
                use super::*;

                #[tokio::test]
                async fn join_empty_list_is_ok_and_quiet() {
                    cases::join_empty_list_is_ok_and_quiet($harness().await).await
                }

                #[tokio::test]
                async fn broadcast_without_neighbors_is_ok() {
                    cases::broadcast_without_neighbors_is_ok($harness().await).await
                }
            }
        }
    };
}

pub(crate) use membership_contract;

/// Membership cases: plain functions, so that the compiler checks them without the macro plugged in.
pub(crate) mod cases {
    use crate::transport::net::contract::{must_finish, pid, SyncPair, SEED_A, SEED_B, STEP};
    use crate::transport::net::id::PeerId;
    use crate::transport::net::sync::SwarmEvent;
    use crate::transport::net::sync_backend::{Swarm, SyncBackend};
    use bytes::Bytes;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::broadcast;

    /// Cases topic: each test has its own nodes, so one for all.
    const TOPIC: [u8; 32] = [7; 32];
    /// "Silence" in the "no events" case.
    const QUIET: Duration = Duration::from_millis(300);

    pub(crate) async fn two_nodes_see_each_other_up(pair: SyncPair) {
        // given: A joined topic [7; 32] alone
        let mut a = join(&pair.a, &[]).await;
        // when: B joins the same topic, inviting A
        let mut b = join(&pair.b, &[pid(SEED_A)]).await;
        // then: within STEP A sees NeighborUp(B), B sees NeighborUp(A)
        next_matching(&mut a.events, is_up(pid(SEED_B))).await;
        next_matching(&mut b.events, is_up(pid(SEED_A))).await;
        finish([a, b]);
    }

    pub(crate) async fn broadcast_reaches_neighbor(pair: SyncPair) {
        // given: A and B are neighbors
        let (a, mut b) = neighbors(&pair).await;
        let hello = Bytes::from("hello from a #1");
        // when: A sends broadcast("hello from a #1")
        let sent = must_finish(a.membership.broadcast(hello.clone())).await;
        // then: Ok, B received Received { from: A, msg } within STEP with the same bytes
        assert!(sent.is_ok(), "broadcast returned {sent:?}");
        let got = next_matching(&mut b.events, is_received).await;
        let (from, msg) = received_parts(got);
        assert_eq!(from, pid(SEED_A), "who the message is from");
        assert_eq!(msg, hello, "the message bytes changed");
        finish([a, b]);
    }

    pub(crate) async fn second_subscriber_gets_events(pair: SyncPair) {
        // given: A joined the topic, a second subscriber membership.subscribe() was created before B arrived
        let mut a = join(&pair.a, &[]).await;
        let mut second = a.membership.subscribe();
        // when: B joins the topic, inviting A, and the neighbor link came up
        let mut b = join(&pair.b, &[pid(SEED_A)]).await;
        next_matching(&mut a.events, is_up(pid(SEED_B))).await;
        // then: the second subscriber sees NeighborUp(B) too
        next_matching(&mut second, is_up(pid(SEED_B))).await;
        next_matching(&mut b.events, is_up(pid(SEED_A))).await;
        finish([a, b]);
    }

    pub(crate) async fn join_empty_list_is_ok_and_quiet(pair: SyncPair) {
        // given: a lone A in the topic
        let mut a = join(&pair.a, &[]).await;
        // when: join(&[])
        let joined = must_finish(a.membership.join(&[])).await;
        // then: Ok, no events within 300 ms
        assert!(joined.is_ok(), "join(&[]) returned {joined:?}");
        let event = tokio::time::timeout(QUIET, a.events.recv()).await;
        assert!(event.is_err(), "an event {event:?} arrived after join(&[])");
        finish([a]);
    }

    pub(crate) async fn broadcast_without_neighbors_is_ok(pair: SyncPair) {
        // given: a lone A in the topic
        let a = join(&pair.a, &[]).await;
        // when: broadcast("hello from a #1")
        let sent = must_finish(a.membership.broadcast(Bytes::from("hello from a #1"))).await;
        // then: Ok
        assert!(sent.is_ok(), "broadcast without peers returned {sent:?}");
        finish([a]);
    }

    /// A node joins the topic `TOPIC` with the initial peers `bootstrap`.
    async fn join(backend: &Arc<dyn SyncBackend>, bootstrap: &[PeerId]) -> Swarm {
        match must_finish(backend.join_swarm(TOPIC, bootstrap)).await {
            Ok(swarm) => swarm,
            Err(e) => panic!("join_swarm did not join the topic: {e}"),
        }
    }

    /// A and B are neighbors: B joined with an invitation from A, both received each other's `NeighborUp`.
    async fn neighbors(pair: &SyncPair) -> (Swarm, Swarm) {
        let mut a = join(&pair.a, &[]).await;
        let mut b = join(&pair.b, &[pid(SEED_A)]).await;
        next_matching(&mut a.events, is_up(pid(SEED_B))).await;
        next_matching(&mut b.events, is_up(pid(SEED_A))).await;
        (a, b)
    }

    /// End of a case: the pumps are stopped.
    fn finish<const N: usize>(swarms: [Swarm; N]) {
        swarms
            .iter()
            .filter_map(|s| s.pump.as_ref())
            .for_each(|p| p.abort());
    }

    /// Waits up to `STEP` for an event matching `want`; skips the others. Panics if it does not arrive.
    async fn next_matching(
        events: &mut broadcast::Receiver<SwarmEvent>,
        want: impl Fn(&SwarmEvent) -> bool,
    ) -> SwarmEvent {
        let waiting = async {
            loop {
                match events.recv().await {
                    Ok(event) if want(&event) => return event,
                    Ok(_) => continue,
                    Err(e) => panic!("the event channel is closed or lagged: {e}"),
                }
            }
        };
        match tokio::time::timeout(STEP, waiting).await {
            Ok(event) => event,
            Err(_) => panic!("the expected event did not arrive within {STEP:?}"),
        }
    }

    fn is_up(id: PeerId) -> impl Fn(&SwarmEvent) -> bool {
        move |event| matches!(event, SwarmEvent::NeighborUp(p) if *p == id)
    }

    fn is_received(event: &SwarmEvent) -> bool {
        matches!(event, SwarmEvent::Received { .. })
    }

    /// `from` and the bytes of `Received`; any other event panics.
    fn received_parts(event: SwarmEvent) -> (PeerId, Bytes) {
        match event {
            SwarmEvent::Received { from, msg } => (from, msg),
            other => panic!("expected Received, got {other:?}"),
        }
    }
}
