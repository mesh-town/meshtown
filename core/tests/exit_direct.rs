//! Integration test of "my exit serves others directly": a member's traffic through an exit node goes out
//! from that node with the operating system's sockets, whatever the node's own proxy slot is pointed at.
//!
//! What it does: on the `mem` loopback, host `x` runs two networks side by side — slot A (`xa`, the exit is on)
//! and slot B (`xb`, `route.exits = [y]`) — with `y`, an exit in network B, and `p`, a plain member in
//! network A. The process proxy front is aimed at `xb` through a `ProxySeat`. A counting echo site sits behind
//! both exits (the name `DOMAIN` in their `exit_hosts` tables). The test reads the exit-client registries of
//! `xa`, `xb` and `y`.
//! What it doesn't do: doesn't run the full process host (the slot files, the web page); the front and the seat
//! are assembled by hand, as in `common/socks_pair.rs`.
//! Links: `meshtown_core::proxy::{ProxyFront, ProxySeat}`, `meshtown_core::exit::ask_exit_to_connect`.

#[path = "common/socks_pair.rs"]
mod common;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use meshtown_core::config::config_file::{NetSlotId, ProcessSettings};
use meshtown_core::exit::{self, PeerConnPool};
use meshtown_core::members::network_key::NetworkKey;
use meshtown_core::node::{self, NodeConfig, RunningNode};
use meshtown_core::proxy::route::Route;
use meshtown_core::proxy::{socks5, ProxyFront, ProxySeat};
use meshtown_core::transport::net::PeerId;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::sleep;

use common::{introduce, wait_exit_route, wait_member, DOMAIN};

const WITHIN: Duration = Duration::from_secs(20);
const POLL: Duration = Duration::from_millis(50);
/// How long a counter that must stay at zero is watched.
const QUIET_WINDOW: Duration = Duration::from_millis(500);

mod member_of_my_network_uses_my_exit {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn echo_receives_the_connection() {
        // given: the front is aimed at slot B, whose exit is `y`
        let fx = setup().await;
        // when: `p` opens the echo through the exit of `x` (slot A)
        let _held = fx.when_p_connects_through_x().await;
        // then: the echo accepted exactly that one connection
        fx.then_echo_accepted(1).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn is_a_client_of_x_and_not_of_y() {
        // given: the front is aimed at slot B, whose exit is `y`
        let fx = setup().await;
        // when: `p` opens the echo through the exit of `x` (slot A)
        let _held = fx.when_p_connects_through_x().await;
        // then: `p` is a client of `xa`; `y` has no clients — the exit did not go through the front
        fx.then_clients(&fx.xa, vec![fx.p.id]).await;
        fx.then_never_clients(&fx.y).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn leaves_slot_b_without_clients() {
        // given: the front is aimed at slot B, whose exit is `y`
        let fx = setup().await;
        // when: `p` opens the echo through the exit of `x` (slot A)
        let _held = fx.when_p_connects_through_x().await;
        // then: `xb` serves nobody — `p` is not a member of network B
        fx.then_never_clients(&fx.xb).await;
    }
}

mod front_traffic {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn goes_through_the_exit_chosen_in_slot_b() {
        // given: the front is aimed at slot B, whose exit is `y`
        let fx = setup().await;
        // when: a local client opens the echo through the front
        let _held = fx.when_local_client_connects_through_front().await;
        // then: the echo accepted the connection from `y`, `xb` is `y`'s client, `xa` has none
        fx.then_echo_accepted(1).await;
        fx.then_clients(&fx.y, vec![fx.xb.id]).await;
        fx.then_never_clients(&fx.xa).await;
    }
}

struct Fixture {
    p: RunningNode,
    xa: RunningNode,
    xb: RunningNode,
    y: RunningNode,
    _seat: ProxySeat,
    proxy: SocketAddr,
    accepted: Arc<AtomicUsize>,
    pool: PeerConnPool,
}

/// Network A: `p` and `xa` (exit on). Network B: `xb` (exit `y`) and `y` (exit on). One front aimed at
/// slot B through a seat that knows both of `x`'s slots.
async fn setup() -> Fixture {
    let (site, accepted) = counting_echo().await;
    let key_a = NetworkKey::generate().unwrap();
    let key_b = NetworkKey::generate().unwrap();
    let y = start(exit_node(key_b.clone(), site)).await;
    let xa = start(exit_node(key_a.clone(), site)).await;
    let xb = start(client_node(key_b, vec![y.id])).await;
    let p = start(client_node(key_a, Vec::new())).await;
    join(&xb, &y).await;
    join(&p, &xa).await;
    let route = wait_exit_route(&xb, WITHIN).await;
    assert_eq!(route, Route::Exits(vec![y.id]), "slot B routes through y");
    let seat = seat_aimed_at_b(&xa, &xb).await;
    let proxy = seat
        .front()
        .and_then(|f| f.addr())
        .expect("the front listens");
    Fixture {
        p,
        xa,
        xb,
        y,
        _seat: seat,
        proxy,
        accepted,
        pool: PeerConnPool::default(),
    }
}

impl Fixture {
    /// `p` asks the exit of `x` (slot A) for a connection to the echo; the stream ends keep it open.
    async fn when_p_connects_through_x(&self) -> impl Sized {
        let (send, recv, code) =
            exit::ask_exit_to_connect(&self.p.stack, &self.pool, self.xa.id, DOMAIN, 80)
                .await
                .expect("the request to the exit must reach a response");
        assert_eq!(code, exit::EXIT_CODE_OK, "the exit of x opens the echo");
        (send, recv)
    }

    /// A local client opens the echo through the process proxy front and proves it with a round trip.
    async fn when_local_client_connects_through_front(&self) -> impl Sized {
        let mut stream = socks5::client_connect(self.proxy, DOMAIN, 80)
            .await
            .expect("the front connects through the chosen exit");
        stream.write_all(b"ping").await.unwrap();
        let mut buf = [0u8; 4];
        tokio::time::timeout(WITHIN, stream.read_exact(&mut buf))
            .await
            .expect("the echo answers in time")
            .unwrap();
        assert_eq!(&buf, b"ping", "the echo returns the bytes");
        stream
    }

    async fn then_echo_accepted(&self, want: usize) {
        let deadline = Instant::now() + WITHIN;
        while self.accepted.load(Ordering::SeqCst) < want && Instant::now() < deadline {
            sleep(POLL).await;
        }
        assert_eq!(
            self.accepted.load(Ordering::SeqCst),
            want,
            "connections accepted by the echo"
        );
    }

    /// The node's exit clients become exactly `want` within the limit.
    async fn then_clients(&self, node: &RunningNode, want: Vec<PeerId>) {
        let deadline = Instant::now() + WITHIN;
        while clients_of(node) != want && Instant::now() < deadline {
            sleep(POLL).await;
        }
        assert_eq!(clients_of(node), want, "exit clients of the node");
    }

    /// The node's exit-client registry stays empty for the whole quiet window.
    async fn then_never_clients(&self, node: &RunningNode) {
        let deadline = Instant::now() + QUIET_WINDOW;
        while Instant::now() < deadline {
            assert!(
                clients_of(node).is_empty(),
                "the node must have no exit clients: {:?}",
                clients_of(node)
            );
            sleep(POLL).await;
        }
    }
}

fn clients_of(node: &RunningNode) -> Vec<PeerId> {
    node.exit_clients.active_peers_at(Instant::now())
}

/// A node that works as an exit and reaches the echo by `DOMAIN`.
fn exit_node(key: NetworkKey, site: SocketAddr) -> NodeConfig {
    let mut cfg = NodeConfig::for_loopback_test(key);
    cfg.settings.exit_enabled = true;
    cfg.exit_hosts = HashMap::from([(DOMAIN.to_string(), site)]);
    cfg
}

/// A node that is not an exit and sends its proxy traffic through `exits`.
fn client_node(key: NetworkKey, exits: Vec<PeerId>) -> NodeConfig {
    let mut cfg = NodeConfig::for_loopback_test(key);
    cfg.settings.exit_enabled = false;
    cfg.settings.route_exits = exits;
    cfg
}

async fn start(cfg: NodeConfig) -> RunningNode {
    node::start_node(cfg).await.expect("the node starts")
}

/// `a` and `b` know each other as members.
async fn join(a: &RunningNode, b: &RunningNode) {
    introduce(a, b).await;
    wait_member(a, b.id, WITHIN).await;
    wait_member(b, a.id, WITHIN).await;
}

/// A front on a free loopback port and a seat holding both slots of `x`; slot B is chosen.
async fn seat_aimed_at_b(xa: &RunningNode, xb: &RunningNode) -> ProxySeat {
    let slot_a = NetSlotId::parse("aaaaaa").unwrap();
    let slot_b = NetSlotId::parse("bbbbbb").unwrap();
    let settings = ProcessSettings {
        socks_enabled: true,
        socks_port: 0,
        ..ProcessSettings::default()
    };
    let front = ProxyFront::bind(&settings).await.expect("the front binds");
    let seat = ProxySeat::new(Some(front), slot_a.clone());
    seat.node_up(&slot_a, xa.proxy_config());
    seat.node_up(&slot_b, xb.proxy_config());
    seat.choose(slot_b);
    seat
}

/// An echo server that counts accepted connections.
async fn counting_echo() -> (SocketAddr, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let accepted = Arc::new(AtomicUsize::new(0));
    let count = accepted.clone();
    node::output::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            count.fetch_add(1, Ordering::SeqCst);
            node::output::spawn(async move {
                let mut buf = [0u8; 1024];
                while let Ok(n) = socket.read(&mut buf).await {
                    if n == 0 || socket.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
    (addr, accepted)
}
