//! The exit stream wait for a SOCKS5 client: when B does not allow
//! more streams than are already taken, a SOCKS client that left while waiting does not take the stream later;
//! a wait of no longer than `OPEN_STREAM_TIMEOUT` ends with a SOCKS "code 4" refusal; a stream to a silent target
//! is released after `HALF_CLOSE_GRACE_SILENT`, not after `HALF_CLOSE_GRACE`.
//!
//! What it does: two real nodes on loopback (`common::pair_with`), B has a stream limit
//! `STREAMS_LIMIT` per connection, A has exit B chosen as the only one (`route.exits`), so
//! `DOMAIN` and `outside.test` go through B. The target behind B is `echo_site` (closes the socket after EOF,
//! otherwise the stream is held by the `splice` grace period) or `silent_site`. The log is read from the ring
//! `meshtown_core::node::output::log_page`. Two tests are slow real-time tests (`#[ignore]`: they wait 5–10 s of real time),
//! they are run with `cargo nextest run --run-ignored only -E 'binary(socks_stream_wait)'`; network calls
//! run under an external deadline (`NET_LIMIT` 5 s, `DRY_LIMIT` 30 s for the dry ones).
//! What it does not do: does not check `PeerConnPool::open_peer_stream` alone (that is `core/tests/exit_pool_timeout.rs`) and does not
//! touch the HTTP proxy — cancelling the wait there is outside the task's boundary.
//! Links: `meshtown_core::{proxy::socks5, exit::OPEN_STREAM_TIMEOUT, node::output}`; the harness is
//! `core/tests/common/socks_pair.rs`.

#[path = "common/socks_pair.rs"]
mod common;

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use meshtown_core::exit::OPEN_STREAM_TIMEOUT;
use meshtown_core::node::output;
use meshtown_core::node::RunningNode;
use meshtown_core::proxy::route::Route;
use meshtown_core::proxy::socks5::{self, Target};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout};

use common::{echo_site, front_for, pair_with, silent_site, wait_exit_route, DOMAIN};

/// How many streams B allows A on the exit connection (as in `core/tests/exit_pool_timeout.rs`): if
/// the pair does not converge with 1, here it is 2 and `when_slots_taken` holds two SOCKS connections.
const STREAMS_LIMIT: u16 = 1;
/// The external deadline of a regular test's network step: code without a deadline fails the test instead of hanging the run.
const NET_LIMIT: Duration = Duration::from_secs(5);
/// The same for the dry tests that wait real seconds.
const DRY_LIMIT: Duration = Duration::from_secs(30);

mod abandoned_request {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore = "the max_bidi_streams stream limit has no effect on mem: a limiter in MemConn is needed"]
    async fn abandoned_socks_request_does_not_take_a_stream_later() {
        // given: SOCKS #1 to DOMAIN holds the only exit stream
        let fx = setup(echo_site().await).await;
        let first = fx.when_slots_taken().await;
        let mark = ring_mark();

        // when: #2 sends the greeting and a request to outside.test without waiting for the reply and closes
        // after 300 ms
        let second = fx.when_request_sent_unanswered("outside.test").await;
        sleep(Duration::from_millis(300)).await;
        drop(second);

        // then (1): within 2 s a line about outside.test appears in the `[socks]` ring: the client left
        then_ring_gets(
            mark,
            "socks",
            &["outside.test", "client left"],
            Duration::from_secs(2),
        )
        .await;

        // when: #1 is closed, #3 goes to DOMAIN
        drop(first);
        let third = fx.when_socks_ok(DOMAIN, NET_LIMIT).await;

        // then (2): #3 got REP_OK within ≤ 5 s (when_socks_ok); (3) the exit did not open outside.test
        then_ring_lacks(mark, "exit", "outside.test");
        drop(third);
    }
}

mod stream_wait_expired {
    use super::*;

    #[ignore = "dry run: waits for OPEN_STREAM_TIMEOUT"]
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn stream_wait_refuses_socks_after_timeout() {
        // given: SOCKS #1 holds the only exit stream
        let fx = setup(echo_site().await).await;
        let _first = fx.when_slots_taken().await;
        let mark = ring_mark();

        // when: #2 asks for DOMAIN and waits for a stream
        let started = Instant::now();
        let refusal = fx.when_socks_refused(DOMAIN, DRY_LIMIT).await;
        let took = started.elapsed();

        // then: a "code 4" refusal after `OPEN_STREAM_TIMEOUT`…+3 s, in the `[route]` ring — "waiting for a stream: 1"
        assert!(
            refusal.contains("the proxy refused, code 4"),
            "not a \"code 4\" refusal: {refusal}"
        );
        assert!(
            took >= OPEN_STREAM_TIMEOUT && took <= OPEN_STREAM_TIMEOUT + Duration::from_secs(3),
            "the refusal came in {took:?}, expected {OPEN_STREAM_TIMEOUT:?}…+3 s"
        );
        then_ring_gets(mark, "route", &["waiting for a stream: 1"], Duration::ZERO).await;
        then_ring_gets(
            mark,
            "socks",
            &["exit is busy (stream limit)"],
            Duration::ZERO,
        )
        .await;
    }
}

mod silent_target {
    use super::*;

    #[ignore = "dry run: waits for HALF_CLOSE_GRACE_SILENT"]
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn silent_target_frees_stream_after_grace_silent() {
        // given: the target is silent; SOCKS #1 to DOMAIN got REP_OK and closed without bytes
        let fx = setup(silent_site().await).await;
        let first = fx.when_slots_taken().await;
        drop(first);

        // when: #2 goes to DOMAIN
        let started = Instant::now();
        let second = fx.when_socks_ok(DOMAIN, DRY_LIMIT).await;
        let took = started.elapsed();

        // then: REP_OK within ≤ 8 s (with `HALF_CLOSE_GRACE` of 60 s on zero bytes there would be a "code 4" refusal after 10 s)
        assert!(
            took <= Duration::from_secs(8),
            "the silent target's stream was released in {took:?}, expected ≤ 8 s"
        );
        drop(second);
    }
}

struct Fixture {
    /// The nodes live as long as the fixture does.
    _nodes: (RunningNode, RunningNode),
    proxy: SocketAddr,
    _front: meshtown_core::proxy::ProxyFront,
}

/// A pair of nodes with a stream limit at B, B's exit chosen at A, targets behind B are `site`.
async fn setup(site: SocketAddr) -> Fixture {
    let (a, b) = pair_with(site, true, |_| {}).await;
    a.set_route_exits(vec![b.id]);
    let route = wait_exit_route(&a, Duration::from_secs(20)).await;
    assert_eq!(
        route,
        Route::Exits(vec![b.id]),
        "A's route must lead through B"
    );
    let front = front_for(&a).await;
    let proxy = front.addr().expect("the SOCKS5 server must be up");
    Fixture {
        _front: front,
        _nodes: (a, b),
        proxy,
    }
}

impl Fixture {
    /// Takes B's whole limit with SOCKS connections to `DOMAIN` (each reaches `REP_OK`).
    async fn when_slots_taken(&self) -> Vec<TcpStream> {
        let mut held = Vec::new();
        for _ in 0..STREAMS_LIMIT {
            held.push(self.when_socks_ok(DOMAIN, NET_LIMIT).await);
        }
        held
    }

    /// A SOCKS connection to `domain`; it must reach `REP_OK` within `limit`.
    async fn when_socks_ok(&self, domain: &str, limit: Duration) -> TcpStream {
        let connected = timeout(limit, socks5::client_connect(self.proxy, domain, 80))
            .await
            .unwrap_or_else(|_| panic!("SOCKS to {domain} did not reach a reply within {limit:?}"));
        connected
            .unwrap_or_else(|e| panic!("SOCKS to {domain} without REP_OK within {limit:?}: {e:#}"))
    }

    /// A SOCKS connection to `domain` that must end with a refusal within `limit`; returns the text
    /// of the client's error.
    async fn when_socks_refused(&self, domain: &str, limit: Duration) -> String {
        let connected = timeout(limit, socks5::client_connect(self.proxy, domain, 80))
            .await
            .unwrap_or_else(|_| {
                panic!("SOCKS to {domain} did not return within {limit:?}: no deadline")
            });
        match connected {
            Ok(_) => panic!("SOCKS to {domain} went through, but a refusal was expected"),
            Err(e) => format!("{e:#}"),
        }
    }

    /// The greeting and a request to connect to `domain:80`, the reply to the request is not read; the socket
    /// is returned open.
    async fn when_request_sent_unanswered(&self, domain: &str) -> TcpStream {
        let mut stream = TcpStream::connect(self.proxy).await.unwrap();
        let hello = [socks5::VERSION, 1, socks5::METHOD_NO_AUTH];
        stream.write_all(&hello).await.unwrap();
        let mut answer = [0u8; 2];
        stream.read_exact(&mut answer).await.unwrap();
        let request =
            socks5::encode_socks5_request(&Target::Domain(domain.to_string()), 80).unwrap();
        stream.write_all(&request).await.unwrap();
        stream
    }
}

/// The number of the log ring's last line: lines after it are new for the scenario.
fn ring_mark() -> u64 {
    output::log_page(0, usize::MAX).last_seq
}

/// Whether there is a line after `mark` with the tag `tag` that contains all of `words`.
fn ring_has(mark: u64, tag: &str, words: &[&str]) -> bool {
    output::log_page(mark, usize::MAX)
        .lines
        .iter()
        .any(|line| line.tag == tag && words.iter().all(|word| line.text.contains(word)))
}

/// Waits up to `limit` for a ring line (`Duration::ZERO` — check once, without waiting).
async fn then_ring_gets(mark: u64, tag: &str, words: &[&str], limit: Duration) {
    let deadline = Instant::now() + limit;
    while !ring_has(mark, tag, words) {
        assert!(
            Instant::now() < deadline,
            "no line with the words {words:?} in the [{tag}] ring within {limit:?}"
        );
        sleep(Duration::from_millis(50)).await;
    }
}

fn then_ring_lacks(mark: u64, tag: &str, word: &str) {
    assert!(
        !ring_has(mark, tag, &[word]),
        "the [{tag}] ring has a line with the word {word:?}: someone else's request took the stream"
    );
}
