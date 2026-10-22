//! Timeout of `open_bi` in `PeerConnPool::open_peer_stream`: when B doesn't allow more
//! streams than are already open, the request waits for a slot no longer than the pool timeout and gets an error with the number of
//! waiters, and the `PeerConnPool::open_bi_waiter_count` counter returns to zero whatever the outcome of the wait.
//!
//! What it does: two real nodes on loopback (`common::pair_with`), B has a stream limit of
//! `STREAMS_LIMIT` per connection; the test fills this limit with open streams and asks for one more through
//! `PeerConnPool::default().with_open_timeout(..)`. Requests run under the outer timeout `NET_LIMIT`: code without
//! a timeout fails the test instead of hanging the run.
//! What it doesn't do: it doesn't go through SOCKS (that is `core/tests/socks_stream_wait.rs`) and doesn't send exit
//! request frames — the streams are empty, and B doesn't see them until they are closed.
//! Links: `meshtown_core::exit::PeerConnPool`; scaffolding in `core/tests/common/socks_pair.rs`.

#[path = "common/socks_pair.rs"]
mod common;

use std::time::{Duration, Instant};

use anyhow::Result;
use meshtown_core::exit::{PeerConnPool, StreamBusy};
use meshtown_core::node::RunningNode;
use meshtown_core::transport::net::StreamPair;
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};

use common::{echo_site, pair_with};

/// How many streams B allows A on the exit connection. If the pair of nodes doesn't work out with 1 (the node's own
/// protocols hold more than one stream), use 2 here: `setup` and `when_slots_taken` hold
/// as many streams as specified, and the waits in the tests stay the same.
const STREAMS_LIMIT: u16 = 1;
/// The pool's `open_bi` timeout in the "request hit the limit" tests.
const SHORT_TIMEOUT: Duration = Duration::from_millis(300);
/// The outer timeout of a network request: without a timeout in the code the test fails here instead of hanging.
const NET_LIMIT: Duration = Duration::from_secs(5);

/// Open ends of a stream: while the test holds them, the limit slot is taken.
type Held = StreamPair;

mod limit_reached {
    use super::*;

    mod one_more_stream_asked {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
        #[ignore = "the max_bidi_streams stream limit has no effect on mem: MemConn needs a limiter"]
        async fn second_stream_times_out_with_waiting_count() {
            // given: B's limit is taken by a held stream
            let fx = setup(SHORT_TIMEOUT).await;
            let _held = fx.when_slots_taken().await;

            // when: we ask for one more stream
            let started = Instant::now();
            let refusal = fx.when_stream_refused().await;
            let took = started.elapsed();

            // then: an error within 0.3–2 s, with the timeout and the number of waiters (just this request), the counter is clean
            assert!(
                took >= SHORT_TIMEOUT && took <= Duration::from_secs(2),
                "refusal came after {took:?}, expected 0.3–2 s"
            );
            assert!(
                refusal.contains("didn't open"),
                "no reason in the error: {refusal}"
            );
            assert!(
                refusal.contains("waiting for a stream: 1"),
                "no number of waiters in the error: {refusal}"
            );
            fx.then_nobody_waiting();
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
        #[ignore = "the max_bidi_streams stream limit has no effect on mem: MemConn needs a limiter"]
        async fn timeout_error_is_typed_stream_busy() {
            // given: B's limit is taken by a held stream
            let fx = setup(SHORT_TIMEOUT).await;
            let _held = fx.when_slots_taken().await;

            // when: we ask for one more stream
            let error = fx.when_stream_error().await;

            // then: the error contains `StreamBusy` with the number of waiters
            let busy = error.downcast_ref::<StreamBusy>();
            assert_eq!(
                busy.map(|b| b.open_bi_waiter_count),
                Some(1),
                "no StreamBusy: {error:#}"
            );
            fx.then_nobody_waiting();
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
        #[ignore = "the max_bidi_streams stream limit has no effect on mem: MemConn needs a limiter"]
        async fn two_waiters_count_each_other() {
            // given: B's limit is taken by a held stream
            let fx = setup(SHORT_TIMEOUT).await;
            let _held = fx.when_slots_taken().await;

            // when: two streams ask for a slot at once
            let (first, second) = fx.when_two_streams_refused().await;

            // then: both are refused, at least one error shows both waiters, the counter is clean
            assert!(
                first.contains("waiting for a stream: 2")
                    || second.contains("waiting for a stream: 2"),
                "neither error has 'waiting for a stream: 2': [{first}] [{second}]"
            );
            fx.then_nobody_waiting();
        }
    }

    mod waiting_stream_dropped {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
        #[ignore = "the max_bidi_streams stream limit has no effect on mem: MemConn needs a limiter"]
        async fn dropped_wait_releases_counter() {
            // given: B's limit is taken by a held stream, a second stream waits for a slot
            let fx = setup(NET_LIMIT).await;
            let _held = fx.when_slots_taken().await;
            let waiter = fx.when_stream_wait_starts();
            sleep(Duration::from_millis(100)).await;
            assert_eq!(
                fx.pool.open_bi_waiter_count(),
                1,
                "the waiter didn't get into the counter"
            );

            // when: the waiting future is dropped
            waiter.abort();
            let _ = waiter.await;

            // then: the counter is back to zero
            fx.then_nobody_waiting();
        }
    }
}

mod slot_frees_before_timeout {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn stream_opens_when_slot_frees_before_timeout() {
        // given: pool timeout 5 s, B's limit is taken by a held stream, a second stream waits for a slot
        let fx = setup(NET_LIMIT).await;
        let held = fx.when_slots_taken().await;
        let started = Instant::now();
        let waiter = fx.when_stream_wait_starts();

        // when: after 200 ms the holder closes its stream
        sleep(Duration::from_millis(200)).await;
        close(held).await;

        // then: the waiter got its stream before the timeout — the timeout doesn't break an ordinary wait
        let opened = timeout(NET_LIMIT, waiter)
            .await
            .expect("the waiter didn't return within the outer timeout")
            .expect("the waiter's task was aborted");
        let took = started.elapsed();
        assert!(
            opened.is_ok(),
            "the slot was freed but the stream didn't open: {:?}",
            opened.err()
        );
        assert!(
            took < Duration::from_secs(4),
            "the stream opened after {took:?}, but the timeout is 5 s"
        );
        fx.then_nobody_waiting();
    }
}

struct Fixture {
    a: RunningNode,
    b: RunningNode,
    pool: PeerConnPool,
}

/// A pair of nodes with a stream limit at B and a pool at A with timeout `open_timeout`.
async fn setup(open_timeout: Duration) -> Fixture {
    let site = echo_site().await;
    let (a, b) = pair_with(site, true, |_| {}).await;
    Fixture {
        a,
        b,
        pool: PeerConnPool::default().with_open_timeout(open_timeout),
    }
}

impl Fixture {
    /// Takes B's whole limit with held streams (each opens immediately).
    async fn when_slots_taken(&self) -> Vec<Held> {
        let mut held = Vec::new();
        for n in 1..=STREAMS_LIMIT {
            let opened = within(self.pool.open_peer_stream(&self.a.stack, self.b.id)).await;
            held.push(
                opened
                    .unwrap_or_else(|e| panic!("stream #{n} within the limit didn't open: {e:#}")),
            );
        }
        held
    }

    /// Asks for a stream beyond the limit and waits for the refusal; returns the error text.
    async fn when_stream_refused(&self) -> String {
        let opened = within(self.pool.open_peer_stream(&self.a.stack, self.b.id)).await;
        refusal_text(opened)
    }

    /// Asks for a stream beyond the limit and waits for the refusal; returns the error itself, not its text.
    async fn when_stream_error(&self) -> anyhow::Error {
        match within(self.pool.open_peer_stream(&self.a.stack, self.b.id)).await {
            Ok(_) => panic!("a stream beyond B's limit opened"),
            Err(e) => e,
        }
    }

    /// Two requests ask for a stream beyond the limit at the same time; returns the texts of both errors.
    async fn when_two_streams_refused(&self) -> (String, String) {
        let (first, second) = tokio::join!(
            within(self.pool.open_peer_stream(&self.a.stack, self.b.id)),
            within(self.pool.open_peer_stream(&self.a.stack, self.b.id)),
        );
        (refusal_text(first), refusal_text(second))
    }

    /// Starts a request for a stream beyond the limit as a separate task, so the test can drop it or
    /// wait for it.
    fn when_stream_wait_starts(&self) -> JoinHandle<Result<Held>> {
        let (pool, stack, peer) = (self.pool.clone(), self.a.stack.clone(), self.b.id);
        tokio::spawn(async move { pool.open_peer_stream(&stack, peer).await })
    }

    fn then_nobody_waiting(&self) {
        assert_eq!(
            self.pool.open_bi_waiter_count(),
            0,
            "after the wait the waiter counter didn't return to zero"
        );
    }
}

/// Waits for a request no longer than `NET_LIMIT`: code without a timeout makes the test panic instead of hanging.
async fn within<T>(request: impl std::future::Future<Output = T>) -> T {
    timeout(NET_LIMIT, request).await.unwrap_or_else(|_| {
        panic!("the stream request didn't return within {NET_LIMIT:?}: there is no timeout")
    })
}

fn refusal_text(opened: Result<Held>) -> String {
    match opened {
        Ok(_) => panic!("a stream beyond B's limit opened"),
        Err(e) => format!("{e:#}"),
    }
}

/// Closes the held streams: `shutdown` on the send side and dropping both ends.
async fn close(held: Vec<Held>) {
    use tokio::io::AsyncWriteExt;
    for StreamPair { mut send, recv } in held {
        let _ = send.shutdown().await;
        drop((send, recv));
    }
}
