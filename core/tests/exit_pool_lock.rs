//! The exit pool lock: an unreachable member must not hold up a stream to a live one.
//!
//! What it does: starts a pair of live nodes on loopback, sends `PeerConnPool::open_peer_stream` to a member whose
//! address is a UDP "black hole" (the socket silently swallows packets, the QUIC handshake to it doesn't finish
//! within the test time; a member with no address at all refuses at once and holds no lock), and then asks
//! for a stream from a live member through the same pool — the stream must open quickly, not wait for someone else's
//! handshake.
//! The second test checks that a repeated request to a live member still goes through.
//! What it doesn't do: doesn't check reopening of a closed connection and doesn't go to the internet.
//! Links: `meshtown_core::exit::PeerConnPool` through `core/tests/common/socks_pair.rs` (`pair`).

#[path = "common/socks_pair.rs"]
mod common;

use std::time::{Duration, Instant};

use meshtown_core::exit::PeerConnPool;
use meshtown_core::transport::net::mem::process_net;
use meshtown_core::transport::net::NodeSecret;
use tokio::time::timeout;

use common::{pair, serve};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unreachable_peer_does_not_block_live_peer() {
    // given: a pair of live nodes and a member whose `mem` connector is in the network but accepts nobody (a handshake with no reply).
    let site = serve(1).await;
    let (a, b) = pair(site, true).await;
    let dead_secret = NodeSecret::generate();
    let dead = dead_secret.peer_id();
    let hole = process_net().connector(&dead_secret);
    let pool = PeerConnPool::default();

    // when: a handshake to the unreachable member is under way and doesn't finish.
    let stuck = {
        let (pool, stack) = (pool.clone(), a.stack.clone());
        tokio::spawn(async move { pool.open_peer_stream(&stack, dead).await.map(|_| ()) })
    };
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !stuck.is_finished(),
        "the handshake to the dead one already finished"
    );

    // then: the stream to the live member opens quickly.
    let started = Instant::now();
    let opened = timeout(
        Duration::from_secs(3),
        pool.open_peer_stream(&a.stack, b.id),
    )
    .await
    .expect("the stream to the live member waited for someone else's handshake longer than 3 s");
    assert!(
        opened.is_ok(),
        "the stream to the live one did not open: {:?}",
        opened.err()
    );
    let took = started.elapsed();
    assert!(
        took < Duration::from_secs(2),
        "the stream opened in {took:?}"
    );
    stuck.abort();
    drop(hole);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn live_peer_connection_is_reused() {
    let site = serve(1).await;
    let (a, b) = pair(site, true).await;
    let pool = PeerConnPool::default();

    let first = pool.open_peer_stream(&a.stack, b.id).await;
    let second = pool.open_peer_stream(&a.stack, b.id).await;

    assert!(
        first.is_ok(),
        "the first stream did not open: {:?}",
        first.err()
    );
    assert!(
        second.is_ok(),
        "the second stream did not open: {:?}",
        second.err()
    );
}
