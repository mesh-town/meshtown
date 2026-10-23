//! Stopping a node under load: while a long stream goes through exit B (A's SOCKS client writes without
//! stopping to `echo_site` behind B), `RunningNode::shutdown` of each node fits into `STOP_LIMIT`,
//! and the `tracing` log has no `ERROR` since the start of the test (the layer is `core/tests/common/log_capture.rs`; events of
//! `tracing` are rare, this is insurance).
//!
//! What it does: a pair of loopback nodes (`socks_pair::pair`), A picks B as the only exit,
//! SOCKS CONNECT to `DOMAIN`, two background jobs — a writer (no pauses) and an echo reader.
//! What it doesn't do: it doesn't check that the stream closed at the client, and doesn't run a stop under packet
//! loss (`core/tests/lossy_link*.rs`).
//! Links: `meshtown_core::node::RunningNode::shutdown`, `meshtown_core::proxy::socks5::client_connect`.

#[path = "common/socks_pair.rs"]
mod common;
#[path = "common/log_capture.rs"]
mod log_capture;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use meshtown_core::node::RunningNode;
use meshtown_core::proxy::route::Route;
use meshtown_core::proxy::socks5;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::time::{sleep, timeout};

use common::{front_for, echo_site, pair, wait_exit_route, DOMAIN};

/// The stop limit of one node under load.
const STOP_LIMIT: Duration = Duration::from_secs(5);
/// How long to wait for traffic to start flowing through the stream.
const FLOW_LIMIT: Duration = Duration::from_secs(10);
/// How many echo bytes must come back before the stop for the stream to count as alive.
const FLOWING_BYTES: usize = 256 * 1024;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn both_nodes_stop_within_limit_while_stream_is_busy() {
    // given: a stream through exit B, the writer doesn't stop, the echo comes back
    let mark = log_capture::mark();
    let (a, b) = pair(echo_site().await, true).await;
    let echoed = when_stream_busy(&a, &b).await;
    wait_flow(&echoed).await;

    // when: we stop the exit, and then the client behind it
    let stop_b = when_stopped(b).await;
    let stop_a = when_stopped(a).await;
    eprintln!("stop of B: {stop_b:?}, stop of A: {stop_a:?}");

    // then: both fit into the limit, no errors in the log
    assert!(stop_b <= STOP_LIMIT, "B stopped in {stop_b:?}");
    assert!(stop_a <= STOP_LIMIT, "A stopped in {stop_a:?}");
    log_capture::assert_no_errors_since(mark);
}

/// Opens a SOCKS stream from A to `DOMAIN` through B, starts a writer and a reader; returns the counter
/// of received echo bytes.
async fn when_stream_busy(a: &RunningNode, b: &RunningNode) -> Arc<AtomicUsize> {
    a.set_route_exits(vec![b.id]);
    let route = wait_exit_route(a, Duration::from_secs(20)).await;
    assert_eq!(
        route,
        Route::Exits(vec![b.id]),
        "A's path must lead through B"
    );
    let front = front_for(a).await;
    let proxy = front.addr().expect("the SOCKS5 server must be up");
    let stream = timeout(FLOW_LIMIT, socks5::client_connect(proxy, DOMAIN, 80))
        .await
        .expect("SOCKS did not answer in time")
        .expect("SOCKS to DOMAIN must reach REP_OK");
    let (read, write) = stream.into_split();
    let echoed = Arc::new(AtomicUsize::new(0));
    meshtown_core::node::output::spawn(write_forever(write));
    meshtown_core::node::output::spawn(read_counting(read, echoed.clone()));
    echoed
}

/// Writes without stopping until the write breaks.
async fn write_forever(mut write: OwnedWriteHalf) {
    let chunk = vec![0xA5u8; 16 * 1024];
    while write.write_all(&chunk).await.is_ok() {}
}

/// Reads the echo and counts bytes until the read breaks.
async fn read_counting(mut read: OwnedReadHalf, echoed: Arc<AtomicUsize>) {
    let mut buf = vec![0u8; 16 * 1024];
    while let Ok(n) = read.read(&mut buf).await {
        if n == 0 {
            return;
        }
        echoed.fetch_add(n, Ordering::Relaxed);
    }
}

/// Waits until `FLOWING_BYTES` of echo come back; if it didn't, it fails.
async fn wait_flow(echoed: &AtomicUsize) {
    let start = Instant::now();
    while echoed.load(Ordering::Relaxed) < FLOWING_BYTES {
        assert!(
            start.elapsed() < FLOW_LIMIT,
            "stream did not speed up: {} B",
            echoed.load(Ordering::Relaxed)
        );
        sleep(Duration::from_millis(50)).await;
    }
}

/// Stops a node under an external limit of twice `STOP_LIMIT`; returns the real time.
async fn when_stopped(node: RunningNode) -> Duration {
    let start = Instant::now();
    timeout(STOP_LIMIT * 2, node.shutdown())
        .await
        .expect("stop did not return even within the double limit");
    start.elapsed()
}
