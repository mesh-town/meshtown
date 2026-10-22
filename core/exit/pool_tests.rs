//! Tests of the pool (`pool.rs`): default and explicitly set service, stream open deadline, waiter counter,
//! cell lifecycle on the mem stack. The `ask_exit_to_connect` request and a hung `open` are in `pool_ask_tests.rs`.

use super::*;
use crate::transport::net::facade::test_support::{pid, secret, within, Probe, Spy};
use crate::transport::net::mem::MemNet;
use crate::transport::net::service::{ServiceId, EXIT};
use crate::transport::net::{NetStack, NetStackBuilder};

#[test]
fn default_uses_exit_service() {
    assert_eq!(PeerConnPool::default().svc, EXIT);
}

#[test]
fn report_tag_marks_exit_pool() {
    assert_eq!(link_stats_tag(EXIT), Some("exit-client"));
}

#[test]
fn report_tag_skips_other_pools() {
    assert_eq!(link_stats_tag(ServiceId::new(b"x/1")), None);
}

#[test]
fn new_keeps_service() {
    assert_eq!(
        PeerConnPool::new(ServiceId::new(b"x/1")).svc,
        ServiceId::new(b"x/1")
    );
}

mod open_timeout {
    use super::*;

    #[test]
    fn new_sets_open_stream_timeout() {
        // given: a pool created by `new` (like `PeerConnPool::default()` in the node)
        let pool = PeerConnPool::new(ServiceId::new(b"x/1"));

        // when: we read the stream open deadline
        let limit = pool.open_timeout;

        // then: it equals `OPEN_STREAM_TIMEOUT`, which is 10 s
        assert_eq!(limit, OPEN_STREAM_TIMEOUT);
        assert_eq!(
            OPEN_STREAM_TIMEOUT,
            Duration::from_secs(10),
            "the deadline is 10 s"
        );
        assert_eq!(PeerConnPool::default().open_timeout, OPEN_STREAM_TIMEOUT);
    }

    #[test]
    fn with_open_timeout_overrides_limit() {
        // given: a pool with the default deadline
        let pool = PeerConnPool::new(ServiceId::new(b"x/1"));

        // when: the deadline is replaced
        let pool = pool.with_open_timeout(Duration::from_millis(300));

        // then: the new deadline, the same service
        assert_eq!(pool.open_timeout, Duration::from_millis(300));
        assert_eq!(pool.svc, ServiceId::new(b"x/1"));
    }
}

mod traffic {
    use super::*;
    use crate::exit::traffic::ExitTraffic;

    #[tokio::test]
    async fn pool_with_traffic_counts_through_peer() {
        // given: a pool with speed counting and an exit to which a stream with data is open
        let traffic = ExitTraffic::default();
        let mut fx = setup(Opts::default());
        fx.pool = fx.pool.with_traffic(traffic.clone());
        let mut pair = within("stream", fx.pool.open_peer_stream(&fx.client, pid(2)))
            .await
            .expect("stream opened");
        tokio::io::AsyncWriteExt::write_all(&mut pair.send, &[7u8; 10_000])
            .await
            .expect("write");

        // when: we wait at most three seconds
        let seen = tokio::time::timeout(Duration::from_secs(3), async {
            while traffic
                .rates_at(std::time::Instant::now())
                .get(&pid(2))
                .is_none_or(|r| r.through_peer_bps == 0)
            {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;

        // then: the exit has a nonzero "through peer" speed
        assert!(seen.is_ok(), "speed did not appear within 3 s");
    }
}

mod waiting_counter {
    use super::*;

    #[test]
    fn waiting_starts_at_zero() {
        // given: a fresh pool, no requests
        let pool = PeerConnPool::default();

        // when / then: nothing is waiting for a stream to open
        assert_eq!(pool.open_bi_waiter_count(), 0);
    }

    #[test]
    fn clones_share_waiting_counter() {
        // given: a pool and its clone (this is how the pool is handed to proxy tasks)
        let pool = PeerConnPool::default();
        let clone = pool.clone();

        // when: the first one starts waiting
        pool.open_bi_waiter_count.fetch_add(1, Ordering::SeqCst);

        // then: the clone sees the same number
        assert_eq!(
            clone.open_bi_waiter_count(),
            1,
            "the waiter counter is shared between clones"
        );
    }
}

mod stream {
    use super::*;

    #[tokio::test]
    async fn second_call_to_same_peer_reuses_connection() {
        // given: an exit (node 2) on the mem network and the client's pool (node 1)
        let fx = setup(Opts::default());

        // when: the pool opens two streams to one peer
        within("first stream", fx.pool.open_peer_stream(&fx.client, pid(2)))
            .await
            .expect("first stream opened");
        within(
            "second stream",
            fx.pool.open_peer_stream(&fx.client, pid(2)),
        )
        .await
        .expect("second stream opened");
        tokio::time::sleep(Duration::from_millis(100)).await;

        // then: one connection — the exit accepted it once
        assert_eq!(fx.probe.calls(), 1, "both streams on one connection");
    }

    #[tokio::test]
    async fn closed_connection_is_replaced_by_new_one() {
        // given: the pool created a connection, the exit closed it
        let fx = setup(Opts::default());
        within("first stream", fx.pool.open_peer_stream(&fx.client, pid(2)))
            .await
            .expect("first stream opened");
        let old = fx.probe.next().await;
        old.close(0, b"done");

        // when: the pool is asked for a stream again (the close does not reach the client at once)
        within("new connection", fx.stream_until_second_connection()).await;

        // then: the exit has a new connection, not the same one
        let new = fx.probe.next().await;
        assert_ne!(
            new.stable_id(),
            old.stable_id(),
            "the cell with the closed connection was replaced"
        );
    }

    #[tokio::test]
    async fn failed_connect_leaves_slot_empty() {
        // given: the client doesn't know the peer's address, `connect` gives NoRoute
        let fx = setup(Opts { linked: false });
        let first = within(
            "stream without a route",
            fx.pool.open_peer_stream(&fx.client, pid(2)),
        )
        .await;
        assert!(first.is_err(), "no such peer: the stream didn't open");

        // when: the peer's address became known and the request was repeated
        fx.link();
        let second = within(
            "stream after add_hints",
            fx.pool.open_peer_stream(&fx.client, pid(2)),
        )
        .await;

        // then: the retry succeeds — the failed connect didn't fill the cell
        assert!(second.is_ok(), "the retry after add_hints should succeed");
    }

    #[tokio::test]
    async fn after_stack_close_is_err_not_panic() {
        // given: the client's stack is closed
        let fx = setup(Opts::default());
        fx.client.close().await;

        // when: the pool is asked for a stream
        let res = within(
            "stream after close",
            fx.pool.open_peer_stream(&fx.client, pid(2)),
        )
        .await;

        // then: an error, not a panic
        assert!(res.is_err(), "a closed stack opens no streams");
    }
}

/// Preconditions: whether the client knows the exit's address.
struct Opts {
    linked: bool,
}

impl Default for Opts {
    fn default() -> Self {
        Opts { linked: true }
    }
}

/// The client (node 1), the exit (node 2, the `EXIT` service with a probe handler) and the client's pool.
struct Fx {
    client: NetStack,
    server: NetStack,
    probe: Arc<Probe>,
    pool: PeerConnPool,
}

impl Fx {
    /// The client learns the exit's address.
    fn link(&self) {
        self.client.add_hints(self.server.invite_addrs()[0].clone());
    }

    /// Asks for streams until the exit accepts a second connection.
    async fn stream_until_second_connection(&self) {
        while self.probe.calls() < 2 {
            let _ = self.pool.open_peer_stream(&self.client, pid(2)).await;
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

fn setup(opts: Opts) -> Fx {
    let net = MemNet::new();
    let probe = Probe::new();
    let server = node(&net, 2, Some(probe.clone()));
    let client = node(&net, 1, None);
    let fx = Fx {
        client,
        server,
        probe,
        pool: PeerConnPool::default(),
    };
    if opts.linked {
        fx.link();
    }
    fx
}

/// Node `seed` on the mem network; with a handler it serves `EXIT`.
fn node(net: &Arc<MemNet>, seed: u8, handler: Option<Arc<Probe>>) -> NetStack {
    let transport = Spy::on(net, &secret(seed)).shared();
    let mut builder = NetStackBuilder::new(secret(seed)).transport(transport);
    if let Some(handler) = handler {
        builder = builder.serve(EXIT, handler);
    }
    builder.build().expect("node built")
}
