//! Integration test of the SOCKS5-through-exit case: a SOCKS5 connection
//! going through an exit closes as soon as the exit disappears from `route.exits`; direct connections
//! are not affected by this change.
//!
//! What it does: two real nodes on loopback (`common::pair`), one connection directly (the domain
//! `DOMAIN`, known to node A) and one through exit B (the domain `outside.test`, known only to B —
//! it proves the reply came through the exit); removes `route.exits` at A and checks
//! that the connection through B closed and the direct one did not.
//! What it does not do: does not check the switch to the next candidate, the TUN path and reopening the
//! connection after the exit is removed — outside the task's boundary.
//! Links: `meshtown::route::RouteCtx::wait_until_exit_dropped`, `meshtown::proxy::path::splice_while_chosen`,
//! `meshtown::exit::splice`; the harness is `core/tests/common/socks_pair.rs`.

#[path = "common/socks_pair.rs"]
mod common;

use std::time::Duration;

use meshtown::route::Route;
use meshtown::socks5;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use common::{echo_site, pair, wait_exit_route, DOMAIN};

/// given: a pair of nodes with an echo site behind B; with an empty `route.exits` a direct connection D to
/// `DOMAIN` is open, with exit B chosen a connection E to `outside.test` is open — both prove
/// they work by an echo; when: `a.set_route_exits(vec![])`; then: E closes (EOF/error) within
/// ≤ 5 s, while D keeps answering with an echo as if nothing happened 500 ms later.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn exit_connection_closes_when_exit_unchosen_direct_survives() {
    let site = echo_site().await;
    let (a, b) = pair(site, true).await;

    // given: `route.exits` is empty — the connection to `DOMAIN` goes directly.
    let proxy = a.socks_addr.expect("the SOCKS5 server must be up");
    let mut d = socks5::client_connect(proxy, DOMAIN, 80).await.unwrap();
    echo_round_trip(&mut d, b"ping-direct", Duration::from_secs(5)).await;

    // given: B is chosen as the only exit — the connection to `outside.test` (A does not resolve it itself)
    // goes through B.
    a.set_route_exits(vec![b.id]);
    let route = wait_exit_route(&a, Duration::from_secs(20)).await;
    assert_eq!(route, Route::Exits(vec![b.id]), "the route must lead through B");
    let mut e = socks5::client_connect(proxy, "outside.test", 80)
        .await
        .unwrap();
    echo_round_trip(&mut e, b"ping-exit", Duration::from_secs(5)).await;

    // when: the route at A is removed — exit B is no longer chosen.
    a.set_route_exits(vec![]);

    // then: reading E within ≤ 5 s gives EOF or an error — the connection through the exit is closed.
    let mut buf = [0u8; 64];
    match tokio::time::timeout(Duration::from_secs(5), e.read(&mut buf)).await {
        Ok(Ok(0)) => {}
        Ok(Err(_)) => {}
        Ok(Ok(n)) => panic!("after the exit was removed E got {n} bytes instead of closing"),
        Err(_) => panic!("E did not close within 5 s after the chosen exit was removed"),
    }

    // then: D is a direct connection, removing `route.exits` does not affect it — the echo still goes through.
    tokio::time::sleep(Duration::from_millis(500)).await;
    echo_round_trip(&mut d, b"ping-direct-again", Duration::from_secs(2)).await;
}

/// Writes `msg`, reads exactly as many bytes back within `limit` and checks that it is the same
/// echo. Keeps the connection open (does not finish the stream) — the caller can write/read
/// to it once more.
async fn echo_round_trip(stream: &mut TcpStream, msg: &[u8], limit: Duration) {
    stream
        .write_all(msg)
        .await
        .expect("echo_round_trip: the write failed");
    let mut buf = vec![0u8; msg.len()];
    tokio::time::timeout(limit, stream.read_exact(&mut buf))
        .await
        .expect("echo_round_trip: no echo within the allotted time")
        .expect("echo_round_trip: reading the echo broke off");
    assert_eq!(buf, msg, "the echo must return what was sent");
}
