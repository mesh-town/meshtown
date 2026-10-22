//! Network tests of the exit-client registry (`exit.clients`): the exit node B sees A as a client
//! while A has a successful exit stream,
//! and forgets it when the connection is closed; a refusal (the exit is off,
//! the address is closed by the `address_guard` filter) and a failed connection to a site don't make a client.
//!
//! What it does: two real nodes on loopback (`common::pair`), behind exit B a "silent" site
//! (`common::silent_site`): it accepts connections, holds the sockets and writes and closes
//! nothing, so `splice` on B hangs after the client drops until the grace period ends: a stream with no
//! bytes is held by `HALF_CLOSE_GRACE_SILENT` (5 s), with bytes by `HALF_CLOSE_GRACE` (60 s). The client record does not
//! depend on the stream tail, it goes away only when the connection closes (`ConnGuard`). Reads the registry directly:
//! `b.exit_clients.active_peers_at(..)`.
//! Refusals for closed addresses (loopback, private networks) are checked with literals right in the request frame:
//! the `host_overrides` table bypasses the filter, so a connection failure is the name `CLOSED_DOMAIN`.
//! What it doesn't do: doesn't check the member filter and sorting (that is `build`, the tests
//! `node_ctl/snapshot/build_exit_tests.rs`), doesn't wait the real 30 s of `EXIT_CLIENT_LINGER` — shifts `now`.
//! Links: `meshtown::exit::{ask_exit_to_connect, Clients, EXIT_CLIENT_LINGER}`; plumbing — `tests/common/socks_pair.rs`.

#[path = "common/socks_pair.rs"]
mod common;

use std::time::{Duration, Instant};

use meshtown::exit::{self, PeerConnPool, EXIT_CLIENT_LINGER};
use meshtown::net::{BoxRecv, BoxSend, PeerId};
use meshtown::node::RunningNode;
use tokio::net::TcpListener;
use tokio::time::{sleep, timeout};

use common::{pair, silent_site, CLOSED_DOMAIN};

const LISTED_WITHIN: Duration = Duration::from_secs(5);
/// Less than `HALF_CLOSE_GRACE_SILENT` (5 s): otherwise the test can't tell forgetting by connection from the `splice` tail.
const FORGOTTEN_WITHIN: Duration = Duration::from_secs(2);
const POLL: Duration = Duration::from_millis(100);

/// The open ends of the stream obtained from `ask_exit_to_connect`: while the test holds them, the stream on A's side is not closed.
type Held = (BoxSend, BoxRecv);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exit_lists_client_after_ok_stream() {
    // given: A and B, behind B's exit a silent site
    let fx = setup().await;

    // when: A asks B to open a connection to the site and holds the stream
    let _held = fx.when_ask_ok().await;

    // then: within ≤5 s A appears in B's client list
    fx.then_listed(Duration::ZERO, true, LISTED_WITHIN).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exit_forgets_client_after_connection_close() {
    // given: A holds an open stream through B and is already visible as a client
    let fx = setup().await;
    let _held = fx.when_ask_ok().await;
    fx.then_listed(Duration::ZERO, true, LISTED_WITHIN).await;

    // when: A closes its node — the connection to B drops, the stream on B is still "open"
    fx.a.stack.close().await;

    // then: within ≤2 s after EXIT_CLIENT_LINGER A is gone from B's list — the record is held by the connection, not by the `splice` tail
    // (a stream with no bytes is held by `HALF_CLOSE_GRACE_SILENT`, 5 s)
    fx.then_listed(EXIT_CLIENT_LINGER + Duration::from_secs(1), false, FORGOTTEN_WITHIN)
        .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refused_and_failed_are_not_clients() {
    // given: A and B, on B the name `CLOSED_DOMAIN` leads to a port with no listener
    let fx = setup().await;

    // when: the exit on B is off, A asks for a connection
    fx.b.set_exit_enabled(false);
    let (_held, code) = fx.when_ask("outside.test", 80).await;

    // then: refusal, and A didn't become a client
    assert_eq!(code, exit::EXIT_CODE_REFUSED, "a disabled exit refuses");
    fx.then_never_listed().await;

    // when: the exit is on again, but the site at the address isn't listening
    fx.b.set_exit_enabled(true);
    let (_held, code) = fx.when_ask(CLOSED_DOMAIN, 80).await;

    // then: the connection didn't open, and A didn't become a client
    assert_eq!(
        code,
        exit::EXIT_CODE_CONNECT_FAILED,
        "a closed port is a connection failure"
    );
    fx.then_never_listed().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn loopback_literal_is_refused_before_connect() {
    // given: A and B; on B's exit loopback a listener lives that accepts nothing itself
    let fx = setup().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    for literal in ["127.0.0.1", "::ffff:127.0.0.1"] {
        // when: A asks B to open a connection to the exit's loopback
        let (_held, code) = fx.when_ask(literal, port).await;

        // then: refusal with code 7, the connection didn't reach the listener, A didn't become a client
        assert_eq!(
            code,
            exit::EXIT_CODE_ADDRESS_FORBIDDEN,
            "{literal}: loopback — the exit's own services are closed"
        );
        let accepted = timeout(Duration::from_millis(300), listener.accept()).await;
        assert!(
            accepted.is_err(),
            "{literal}: the site on loopback received no connection"
        );
        fx.then_never_listed().await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_literal_is_refused_before_network() {
    // given: A and B
    let fx = setup().await;

    for literal in ["192.168.1.1", "10.0.0.1", "172.16.0.1"] {
        // when: A asks B to open a connection to a private address of the exit's owner
        let (_held, code) = fx.when_ask(literal, 80).await;

        // then: refusal with code 7 immediately (the exit doesn't touch the networks), A didn't become a client
        assert_eq!(
            code,
            exit::EXIT_CODE_ADDRESS_FORBIDDEN,
            "{literal}: the exit owner's private network is closed"
        );
        fx.then_never_listed().await;
    }
}

struct Fixture {
    a: RunningNode,
    b: RunningNode,
    pool: PeerConnPool,
}

async fn setup() -> Fixture {
    let (a, b) = pair(silent_site().await, true).await;
    Fixture {
        a,
        b,
        pool: PeerConnPool::default(),
    }
}

impl Fixture {
    /// A asks B to open a connection to `domain:port`; returns the stream ends and the response code.
    async fn when_ask(&self, domain: &str, port: u16) -> (Held, u8) {
        let (send, recv, code) = exit::ask_exit_to_connect(&self.a.stack, &self.pool, self.b.id, domain, port)
            .await
            .expect("the request to the exit must reach a response");
        ((send, recv), code)
    }

    /// The same for the success case: a domain known to B, the response must be `EXIT_CODE_OK`.
    async fn when_ask_ok(&self) -> Held {
        let (held, code) = self.when_ask("outside.test", 80).await;
        assert_eq!(
            code,
            exit::EXIT_CODE_OK,
            "B's exit must open a connection to the site"
        );
        held
    }

    /// Polls B's registry at the moment "now + `shift`" until `A ∈ list` becomes equal to
    /// `listed`; if it doesn't converge within `limit` — panic.
    async fn then_listed(&self, shift: Duration, listed: bool, limit: Duration) {
        let deadline = Instant::now() + limit;
        let what = if listed {
            "did not appear"
        } else {
            "did not leave"
        };
        while list_has(&self.b, self.a.id, shift) != listed {
            assert!(
                Instant::now() < deadline,
                "within {limit:?} (shift {shift:?}) A {what} in B's clients"
            );
            sleep(POLL).await;
        }
    }

    /// Watches B's registry for a short window: A never got into the list.
    async fn then_never_listed(&self) {
        let deadline = Instant::now() + Duration::from_millis(400);
        while Instant::now() < deadline {
            assert!(
                !list_has(&self.b, self.a.id, Duration::ZERO),
                "A must not be a client of B"
            );
            sleep(POLL).await;
        }
    }
}

fn list_has(node: &RunningNode, who: PeerId, shift: Duration) -> bool {
    node.exit_clients
        .active_peers_at(Instant::now() + shift)
        .contains(&who)
}
