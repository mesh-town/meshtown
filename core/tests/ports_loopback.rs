//! Integration tests for port publishing: two real nodes on loopback, A publishes a
//! TCP port, B opens it locally on `127.0.0.1:<local>`.
//!
//! What it does: checks the end-to-end path (an entry in the document → read at B → QUIC stream → echo),
//! unpublishing, a slash in the key (remove 80 without touching 8080), a `Nodes` refusal, the
//! "not responding" status on the publisher and closing an opened port.
//! What it does not do: does not check the page and the config file.
//! Links: `meshtown::ports`, `meshtown::node`; the harness is `core/tests/common/socks_pair.rs`.

#[path = "common/socks_pair.rs"]
mod common;

use std::time::Duration;

use meshtown::network_key::NetworkKey;
use meshtown::node::{self, NodeConfig, RunningNode};
use meshtown::ports::{Audience, LocalState, Opened, Published};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{sleep, timeout, Instant};

use common::{echo_site, introduce, wait_member};

fn all(port: u16, name: &str) -> Published {
    Published {
        port,
        name: name.to_string(),
        audience: Audience::All,
    }
}

async fn free_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    l.local_addr().unwrap().port()
}

/// A with the publications `published`, B without them; both know each other.
async fn pair_ports(published: Vec<Published>) -> (RunningNode, RunningNode) {
    let key = NetworkKey::generate().unwrap();
    let mut cfg_a = NodeConfig::for_loopback_test(key.clone());
    cfg_a.settings.exit_enabled = false;
    cfg_a.socks.enabled = false;
    cfg_a.settings.ports_published = published;
    let mut cfg_b = NodeConfig::for_loopback_test(key);
    cfg_b.settings.exit_enabled = false;
    cfg_b.socks.enabled = false;
    let a = node::start_node(cfg_a).await.unwrap();
    let b = node::start_node(cfg_b).await.unwrap();
    introduce(&a, &b).await;
    wait_member(&a, b.id, Duration::from_secs(20)).await;
    wait_member(&b, a.id, Duration::from_secs(20)).await;
    (a, b)
}

/// Waits until `cond` becomes true, no longer than `limit`.
async fn wait_for<F, Fut>(limit: Duration, what: &str, mut cond: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        if cond().await {
            return;
        }
        sleep(Duration::from_millis(100)).await;
    }
    panic!("timed out after {limit:?}: {what}");
}

/// The ports of author `a` that node `b` sees in the document (readable entries only).
async fn seen_ports(b: &RunningNode, a: &RunningNode) -> Vec<u16> {
    let mut v: Vec<u16> = b
        .ports_doc
        .read_all()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.author == a.id && p.record.is_some())
        .map(|p| p.port)
        .collect();
    v.sort_unstable();
    v
}

/// The connection to `127.0.0.1:local` is closed without data: EOF or a read error.
async fn assert_closed_without_data(local: u16) {
    let mut s = TcpStream::connect(("127.0.0.1", local)).await.unwrap();
    let _ = s.write_all(b"ping").await;
    let mut buf = [0u8; 16];
    let r = timeout(Duration::from_secs(12), s.read(&mut buf))
        .await
        .expect("no reply");
    assert!(matches!(r, Ok(0) | Err(_)), "data arrived: {r:?}");
}

fn opened(a: &RunningNode, port: u16, local: u16) -> Vec<Opened> {
    vec![Opened {
        node: a.id,
        port,
        local,
    }]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn open_carries_echo_and_shows_record() {
    // given: A publishes echo_site for everyone, B opened it on a free port.
    let site = echo_site().await;
    let (a, b) = pair_ports(vec![all(site.port(), "echo")]).await;
    let local = free_port().await;
    b.set_ports_opened(opened(&a, site.port(), local));

    // when B reads the document / then it sees A's entry with this port.
    wait_for(Duration::from_secs(10), "A's entry at B", || async {
        seen_ports(&b, &a).await == vec![site.port()]
    })
    .await;

    // when a client writes to 127.0.0.1:<local> at B / then it gets the same bytes back.
    let mut s = TcpStream::connect(("127.0.0.1", local)).await.unwrap();
    s.write_all(b"hello ports").await.unwrap();
    let mut buf = [0u8; 11];
    timeout(Duration::from_secs(10), s.read_exact(&mut buf))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&buf, b"hello ports");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn removed_publication_refuses_new_connections() {
    // given: A unpublished.
    let site = echo_site().await;
    let (a, b) = pair_ports(vec![all(site.port(), "echo")]).await;
    let local = free_port().await;
    b.set_ports_opened(opened(&a, site.port(), local));
    wait_for(Duration::from_secs(10), "A's entry at B", || async {
        seen_ports(&b, &a).await == vec![site.port()]
    })
    .await;
    a.set_ports_published(vec![]);
    wait_for(Duration::from_secs(10), "the unpublishing at A", || async {
        a.ports_doc.own_ports().await.unwrap().is_empty()
    })
    .await;

    // when a new connection to B / then it is closed without data.
    assert_closed_without_data(local).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn removing_80_keeps_8080() {
    // given: A publishes 80 and 8080, kept 8080.
    let (a, b) = pair_ports(vec![all(80, "one"), all(8080, "two")]).await;
    wait_for(Duration::from_secs(10), "both entries at B", || async {
        seen_ports(&b, &a).await == vec![80, 8080]
    })
    .await;
    a.set_ports_published(vec![all(8080, "two")]);

    // when the announcer has run / then own = [8080], B has 8080 and not 80 (the key's slash).
    wait_for(Duration::from_secs(10), "own = [8080]", || async {
        a.ports_doc.own_ports().await.unwrap() == vec![8080]
    })
    .await;
    wait_for(Duration::from_secs(10), "only 8080 at B", || async {
        seen_ports(&b, &a).await == vec![8080]
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn nodes_audience_is_refused_for_unlisted_node() {
    // given: A publishes with Nodes([A]) — B is not in the list.
    let site = echo_site().await;
    let p = Published {
        port: site.port(),
        name: "private".into(),
        audience: Audience::Nodes(vec![]),
    };
    let (a, b) = pair_ports(vec![p]).await;
    let p = Published {
        audience: Audience::Nodes(vec![a.id]),
        ..all(site.port(), "private")
    };
    a.set_ports_published(vec![p]);
    let local = free_port().await;
    b.set_ports_opened(opened(&a, site.port(), local));
    wait_for(Duration::from_secs(10), "the entry at B", || async {
        !seen_ports(&b, &a).await.is_empty()
    })
    .await;

    // when a connection to B / then it is closed without data (a node outside the Nodes list gets a refusal).
    assert_closed_without_data(local).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unlistened_port_is_closed_and_marked_refused() {
    // given: A publishes a port nobody listens on.
    let dead = free_port().await;
    let (a, b) = pair_ports(vec![all(dead, "dead")]).await;
    let local = free_port().await;
    b.set_ports_opened(opened(&a, dead, local));
    wait_for(Duration::from_secs(10), "the entry at B", || async {
        seen_ports(&b, &a).await == vec![dead]
    })
    .await;

    // when a connection to B / then it is closed; A.ports.published_with_state() → Refused.
    assert_closed_without_data(local).await;
    wait_for(Duration::from_secs(5), "the Refused status", || async {
        a.ports
            .published_with_state()
            .iter()
            .any(|(p, s)| p.port == dead && *s == LocalState::Refused)
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn closed_local_port_stops_listening() {
    // given: B closed the port (set_ports_opened([])).
    let site = echo_site().await;
    let (a, b) = pair_ports(vec![all(site.port(), "echo")]).await;
    let local = free_port().await;
    b.set_ports_opened(opened(&a, site.port(), local));
    assert!(TcpStream::connect(("127.0.0.1", local)).await.is_ok());
    b.set_ports_opened(vec![]);

    // when connect to 127.0.0.1:<local> / then it is rejected (with retries for up to 1 s).
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        if TcpStream::connect(("127.0.0.1", local)).await.is_err() {
            return;
        }
        assert!(Instant::now() < deadline, "the port is still listening");
        sleep(Duration::from_millis(50)).await;
    }
}
