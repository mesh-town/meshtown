//! The shared harness for SOCKS5/exit scenarios: a pair of real nodes on loopback (`pair`, `pair_with` —
//! with a tweak of B's settings, `pair_with_static_exit`), the handshake (`introduce`, `wait_member`),
//! waiting for the route through an exit (`wait_exit_route`) and local target servers (`serve`, `echo_site`,
//! `silent_site`). Plugged in as `#[path = "common/socks_pair.rs"] mod common;` from the
//! SOCKS5 and exit tests (`tests/socks_*.rs`, `tests/exit_*.rs`, `core/tests/ports_loopback.rs`,
//! `core/tests/http_proxy_loopback.rs`). Each scenario uses only part of this module — the rest is not
//! dead code of the project but functions for a neighboring file: `#![allow(dead_code)]` below is about that, not
//! about tolerating real garbage.
#![allow(dead_code)]

use meshtown_core::transport::net::{PeerAddr, PeerId};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use meshtown_core::members::network_key::NetworkKey;
use meshtown_core::node::{self, NodeConfig, RunningNode};
use meshtown_core::proxy::route::Route;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{sleep, Instant};

pub(crate) const DOMAIN: &str = "site.test";

/// A name that B's exit `host_overrides` leads to port 1 of the loopback with no listener: the exit opens
/// a connection and gets a refusal (`EXIT_CODE_CONNECT_FAILED`). The literal 127.0.0.1 will not do for this —
/// the exit closes the loopback by filtering addresses (code 7), the `host_overrides` table bypasses it.
pub(crate) const CLOSED_DOMAIN: &str = "closed.test";

/// A server that answers any request with 200 and a body of `body_kb` kilobytes.
pub(crate) async fn serve(body_kb: usize) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    meshtown_core::node::output::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            meshtown_core::node::output::spawn(async move {
                let mut buf = vec![0u8; 2048];
                let _ = socket.read(&mut buf).await;
                let len = body_kb * 1024;
                let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {len}\r\n\r\n");
                if socket.write_all(head.as_bytes()).await.is_err() {
                    return;
                }
                let chunk = vec![b'x'; 1024];
                for _ in 0..body_kb {
                    if socket.write_all(&chunk).await.is_err() {
                        return;
                    }
                }
                let _ = socket.shutdown().await;
            });
        }
    });
    addr
}

/// A local TCP server: echo. Reads what arrived, sends the same thing straight back, and so
/// in a loop — keeps the connection open until the client sends EOF (closes its side of the
/// writing) or until a write/read breaks with an error. For the `socks_exit_reset` test: a connection
/// that the client keeps open is not closed by the server on its own — it can be closed
/// only by interrupting the path (the exit removed) or by the client explicitly breaking off.
pub(crate) async fn echo_site() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    meshtown_core::node::output::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            meshtown_core::node::output::spawn(async move {
                let mut buf = vec![0u8; 4096];
                loop {
                    let n = match socket.read(&mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => n,
                    };
                    if socket.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
                let _ = socket.shutdown().await;
            });
        }
    });
    addr
}

/// A "silent" site: accepts connections, stores the sockets and holds them — writes nothing and
/// closes nothing. Lives as long as the test's runtime lives.
pub(crate) async fn silent_site() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    meshtown_core::node::output::spawn(async move {
        let mut held = Vec::new();
        while let Ok((socket, _)) = listener.accept().await {
            held.push(socket);
        }
    });
    addr
}

pub(crate) async fn introduce(a: &RunningNode, b: &RunningNode) {
    a.introduce(PeerAddr::new(b.id)).await;
}

/// Waits until the node recognizes the given one as a member.
pub(crate) async fn wait_member(node: &RunningNode, who: PeerId, limit: Duration) {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        if node.members.is_member(who) {
            return;
        }
        sleep(Duration::from_millis(50)).await;
    }
    panic!("the handshake did not happen within {limit:?}");
}

/// Waits until `RouteCtx` starts returning a route through the chosen exit: the candidate does not appear
/// at once — B's announcement of itself as an exit (`node_state::run_exit_announce`) must reach
/// A through the shared list.
pub(crate) async fn wait_exit_route(node: &RunningNode, limit: Duration) -> Route {
    let deadline = Instant::now() + limit;
    loop {
        let route = node.route.choose_now().await.unwrap();
        if matches!(route, Route::Exits(_)) || Instant::now() >= deadline {
            return route;
        }
        sleep(Duration::from_millis(50)).await;
    }
}

/// A pair of nodes: B is a member that can work as an exit; A knows about it after the handshake.
/// `route.exits` at A is empty — the caller sets it itself (with the live knob or through
/// `pair_with_static_exit`). `cfg_a.exit_hosts` knows only `DOMAIN` (it proves the direct
/// path) — A does not resolve `outside.test` itself, only B knows it; `CLOSED_DOMAIN` is also only at B.
pub(crate) async fn pair(site: SocketAddr, exit_enabled: bool) -> (RunningNode, RunningNode) {
    pair_with(site, exit_enabled, |_| {}).await
}

/// Like `pair`, but B's settings are edited by `tune_b` before the node starts.
pub(crate) async fn pair_with(
    site: SocketAddr,
    exit_enabled: bool,
    tune_b: impl FnOnce(&mut NodeConfig),
) -> (RunningNode, RunningNode) {
    let key = NetworkKey::generate().unwrap();
    let mut cfg_a = NodeConfig::for_loopback_test(key.clone());
    cfg_a.settings.exit_enabled = false;
    cfg_a.exit_hosts = HashMap::from([(DOMAIN.to_string(), site)]);
    let mut cfg_b = NodeConfig::for_loopback_test(key);
    cfg_b.settings.exit_enabled = exit_enabled;
    cfg_b.socks.enabled = false;
    cfg_b.exit_hosts = HashMap::from([
        (DOMAIN.to_string(), site),
        ("outside.test".to_string(), site),
        (
            CLOSED_DOMAIN.to_string(),
            SocketAddr::from(([127, 0, 0, 1], 1)),
        ),
    ]);
    tune_b(&mut cfg_b);

    let a = node::start_node(cfg_a).await.unwrap();
    let b = node::start_node(cfg_b).await.unwrap();
    introduce(&a, &b).await;
    wait_member(&a, b.id, Duration::from_secs(20)).await;
    wait_member(&b, a.id, Duration::from_secs(20)).await;
    (a, b)
}

/// A pair of nodes where A knows B as the only exit in
/// `route.exits` from the very start (not by a live edit) — this checks the config path `Settings.route_exits` → `node/start/serving.rs` →
/// `RouteCtx::new`, not only the live knob `set_route_exits`.
pub(crate) async fn pair_with_static_exit(site: SocketAddr) -> (RunningNode, RunningNode) {
    let key = NetworkKey::generate().unwrap();
    let mut cfg_b = NodeConfig::for_loopback_test(key.clone());
    cfg_b.settings.exit_enabled = true;
    cfg_b.socks.enabled = false;
    cfg_b.exit_hosts = HashMap::from([(DOMAIN.to_string(), site)]);
    let b = node::start_node(cfg_b).await.unwrap();

    let mut cfg_a = NodeConfig::for_loopback_test(key);
    cfg_a.settings.exit_enabled = false;
    cfg_a.settings.route_exits = vec![b.id];
    let a = node::start_node(cfg_a).await.unwrap();

    introduce(&a, &b).await;
    wait_member(&a, b.id, Duration::from_secs(20)).await;
    wait_member(&b, a.id, Duration::from_secs(20)).await;
    (a, b)
}
