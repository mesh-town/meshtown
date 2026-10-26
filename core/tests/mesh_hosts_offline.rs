//! Integration tests for `<name>.meshtown` addresses of a member that is offline: two real nodes on
//! loopback, A is named `Café Router` and publishes a port, then A is shut down.
//!
//! What it does: a port open to B gives the "offline" answer (503); a port that is not open to B (published
//! for an empty list of nodes) gives the "not shared" answer (403) that also says the owner is offline.
//! What it does not do: does not check the page markup (unit tests of `proxy::mesh::page`).
//! Links: `meshtown_core::proxy`; the harness is `core/tests/common/socks_pair.rs`.

#[path = "common/socks_pair.rs"]
mod common;

use std::net::SocketAddr;
use std::time::Duration;

use meshtown_core::members::network_key::NetworkKey;
use meshtown_core::node::{self, NodeConfig};
use meshtown_core::ports_sharing::{Audience, Proto, Published};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{sleep, Instant};

use common::{front_for, introduce, wait_member};

const NAME: &str = "Café Router";
const PORT: u16 = 18_080;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn port_open_to_b_gets_503_when_a_is_offline() {
    // given: A shares PORT with everyone, B has seen it, A goes away.
    let (status, _) = ask_offline(Audience::All).await;

    // then: the node answers 503.
    assert_eq!(status, 503);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn port_not_open_to_b_gets_403_naming_the_offline_owner() {
    // given: A publishes PORT for an empty list of nodes, B has seen it, A goes away.
    let (status, body) = ask_offline(Audience::Nodes(vec![])).await;

    // then: 403 and the page says the owner is offline too.
    assert_eq!(status, 403, "body: {body}");
    assert!(body.contains("is also offline right now"), "body: {body}");
}

/// Publishes PORT for `audience` on A, stops A once B has read the entry, and asks B's proxy for the
/// address on PORT. Returns the status and the body.
async fn ask_offline(audience: Audience) -> (u16, String) {
    let key = NetworkKey::generate().unwrap();
    let mut cfg_a = NodeConfig::for_loopback_test(key.clone());
    cfg_a.settings.exit_enabled = false;
    cfg_a.settings.node_name = Some(NAME.to_string());
    cfg_a.settings.ports_published = vec![Published {
        proto: Proto::Tcp,
        port: PORT,
        name: "web".into(),
        audience,
    }];
    let mut cfg_b = NodeConfig::for_loopback_test(key);
    cfg_b.settings.exit_enabled = false;
    let a = node::start_node(cfg_a).await.unwrap();
    let b = node::start_node(cfg_b).await.unwrap();
    introduce(&a, &b).await;
    wait_member(&a, b.id, Duration::from_secs(20)).await;
    wait_member(&b, a.id, Duration::from_secs(20)).await;
    wait_for("A's name and entry at B", || {
        b.members.name_of(a.id).as_deref() == Some(NAME)
    })
    .await;
    wait_entry(&b, a.id).await;
    let front = front_for(&b).await;
    let proxy = front.addr().expect("the proxy port must be up");
    std::mem::forget(front);
    let a_id = a.id;
    a.shutdown().await;
    wait_for("B sees A leave", || !b.members.is_member(a_id)).await;
    get(proxy).await
}

async fn wait_entry(b: &node::RunningNode, a: meshtown_core::transport::net::PeerId) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        let seen = b.ports_doc.read_all().await.unwrap_or_default();
        if seen
            .iter()
            .any(|p| p.author == a && p.port == PORT && p.record.is_some())
        {
            return;
        }
        sleep(Duration::from_millis(100)).await;
    }
    panic!("B did not see A's entry within 20 s");
}

async fn wait_for(what: &str, mut cond: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(40);
    while Instant::now() < deadline {
        if cond() {
            return;
        }
        sleep(Duration::from_millis(100)).await;
    }
    panic!("timed out: {what}");
}

/// Absolute-URI `GET` for the address on PORT through the HTTP proxy; returns status and body.
async fn get(proxy: SocketAddr) -> (u16, String) {
    let host = format!("cafe-router.meshtown:{PORT}");
    let request =
        format!("GET http://{host}/ HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    let mut s = TcpStream::connect(proxy).await.unwrap();
    s.write_all(request.as_bytes()).await.unwrap();
    let mut raw = Vec::new();
    let _ = s.read_to_end(&mut raw).await;
    let text = String::from_utf8_lossy(&raw).into_owned();
    let status = text
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    (status, text)
}
