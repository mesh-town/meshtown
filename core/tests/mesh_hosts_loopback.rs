//! Integration tests for `<name>.meshtown` addresses: two real nodes on loopback, A is named
//! `Café Router` (address `cafe-router.meshtown`), B reaches A's published ports through its own proxy.
//!
//! What it does: checks the end-to-end path of the three proxy entries (SOCKS5, HTTP absolute-URI
//! forwarding, HTTP `CONNECT`) to a `.meshtown` address: an open port carries bytes, a port that is not
//! shared (other port, empty `Nodes` list) and an unknown member are answered with an HTML page by the node
//! itself, a TLS handshake aimed at a page is closed without bytes.
//! What it does not do: does not check the page markup and texts (unit tests of `proxy::mesh::page`), name
//! collisions between networks, offline members, the PAC file.
//! Links: `meshtown_core::proxy`, `meshtown_core::ports_sharing`; the harness is `core/tests/common/socks_pair.rs`.

#[path = "common/socks_pair.rs"]
mod common;

use std::net::SocketAddr;
use std::time::Duration;

use meshtown_core::members::network_key::NetworkKey;
use meshtown_core::node::{self, NodeConfig, RunningNode};
use meshtown_core::ports_sharing::{Audience, Proto, Published};
use meshtown_core::proxy::socks5;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout, Instant};

use common::{echo_site, front_for, introduce, serve, wait_member};

/// The name of A; the node turns it into the address below.
const NAME: &str = "Café Router";
const ADDRESS: &str = "cafe-router.meshtown";
const TLS_HELLO_START: [u8; 3] = [0x16, 0x03, 0x01];

mod open_port {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn socks5_carries_echo_back() {
        // given: A shares an echo server with everyone.
        let echo = echo_site().await;
        let mesh = setup(vec![all(echo.port(), "echo")]).await;

        // when: B's client connects to the address through SOCKS5 and writes.
        let mut s = socks5::client_connect(mesh.proxy, ADDRESS, echo.port())
            .await
            .unwrap();
        s.write_all(b"hello mesh").await.unwrap();

        // then: the same bytes come back.
        let mut buf = [0u8; 10];
        timeout(Duration::from_secs(10), s.read_exact(&mut buf))
            .await
            .expect("no echo within 10 s")
            .unwrap();
        assert_eq!(&buf, b"hello mesh");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn http_proxy_returns_the_site_body() {
        // given: A shares a web site (answers 200 with 1 KiB of `x`) with everyone.
        let site = serve(1).await;
        let mesh = setup(vec![all(site.port(), "site")]).await;

        // when: a client sends an absolute-URI request for the address to B's proxy.
        let reply = http_via_proxy(mesh.proxy, &get_through_proxy(site.port())).await;

        // then: the site's answer comes back as is.
        assert_eq!(reply.status, 200, "reply: {}", reply.head);
        assert_eq!(reply.body, "x".repeat(1024), "the body of the site");
    }
}

mod other_port {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn port_80_gets_403_page_naming_the_shared_port() {
        // given: A shares only the echo port P; B has seen A's entry.
        let echo = echo_site().await;
        let mesh = setup(vec![all(echo.port(), "echo")]).await;
        wait_ports_seen(&mesh, echo.port()).await;

        // when: B's client asks for the address on port 80.
        let request = "GET http://cafe-router.meshtown/ HTTP/1.1\r\nHost: cafe-router.meshtown\r\nConnection: close\r\n\r\n";
        let reply = http_via_proxy(mesh.proxy, request).await;

        // then: the node answers 403 and the page offers the shared port.
        assert_eq!(reply.status, 403, "reply: {}", reply.head);
        let offered = format!("{ADDRESS}:{}", echo.port());
        assert!(
            reply.body.contains(&offered),
            "the page must name {offered}: {}",
            reply.body
        );
    }
}

mod empty_nodes_audience {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn nobody_is_let_in_so_the_page_says_403() {
        // given: A publishes P for the explicit list of nodes, and the list is empty.
        let echo = echo_site().await;
        let published = Published {
            proto: Proto::Tcp,
            port: echo.port(),
            name: "echo".to_string(),
            audience: Audience::Nodes(vec![]),
        };
        let mesh = setup(vec![published]).await;

        // when: B's client asks for P.
        let reply = http_via_proxy(mesh.proxy, &get_through_proxy(echo.port())).await;

        // then: the owner refuses everyone, so the node shows the "not shared" page.
        assert_eq!(reply.status, 403, "reply: {}", reply.head);
    }
}

mod socks5_to_unshared_port {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn port_80_is_accepted_then_http_request_gets_403() {
        // given: A shares nothing.
        let mesh = setup(vec![]).await;

        // when: SOCKS5 connect to :80 succeeds (REP 0) and the client sends a request.
        let mut s = socks5::client_connect(mesh.proxy, ADDRESS, 80)
            .await
            .expect("SOCKS5 must reply REP 0 and serve the page");
        s.write_all(origin_form_request().as_bytes()).await.unwrap();

        // then: the page answers 403.
        let reply = parse_reply(&read_to_close(&mut s).await);
        assert_eq!(reply.status, 403, "reply: {}", reply.head);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn port_443_is_accepted_then_tls_hello_is_closed_without_bytes() {
        // given: A shares nothing.
        let mesh = setup(vec![]).await;

        // when: SOCKS5 connect to :443 succeeds (REP 0) and the client starts a TLS handshake.
        let mut s = socks5::client_connect(mesh.proxy, ADDRESS, 443)
            .await
            .expect("SOCKS5 must reply REP 0 and serve the page");
        s.write_all(&TLS_HELLO_START).await.unwrap();

        // then: the node closes the connection without a single byte.
        assert_closed_without_data(&mut s).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn plain_http_on_443_gets_the_page_not_a_tls_error() {
        // given: A shares nothing.
        let mesh = setup(vec![]).await;

        // when: a plain (origin-form) HTTP request goes to :443 of the address through SOCKS5.
        let mut s = socks5::client_connect(mesh.proxy, ADDRESS, 443)
            .await
            .expect("SOCKS5 must reply REP 0 and serve the page");
        s.write_all(origin_form_request().as_bytes()).await.unwrap();

        // then: the node answers with the 403 page about port 443.
        let reply = parse_reply(&read_to_close(&mut s).await);
        assert_eq!(reply.status, 403, "reply: {}", reply.head);
        assert!(
            reply.body.contains("hasn't shared port 443 with you"),
            "the page must say port 443 is not shared: {}",
            reply.body
        );
    }
}

mod connect_tunnel {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn connect_to_443_is_accepted_then_tls_hello_is_closed_without_bytes() {
        // given: A shares nothing.
        let mesh = setup(vec![]).await;

        // when: the client sends CONNECT for the address on 443 and then a TLS byte.
        let (mut s, status) = connect_via(mesh.proxy, "cafe-router.meshtown:443").await;
        s.write_all(&TLS_HELLO_START).await.unwrap();

        // then: the proxy said 200 and the node closes without bytes.
        assert!(status.contains(" 200"), "status line: {status}");
        assert_closed_without_data(&mut s).await;
    }
}

mod unknown_member {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn address_nobody_gets_404_page() {
        // given: the network has only A (and B).
        let mesh = setup(vec![]).await;

        // when: the client asks for a name nobody has.
        let request = "GET http://nobody.meshtown/ HTTP/1.1\r\nHost: nobody.meshtown\r\nConnection: close\r\n\r\n";
        let reply = http_via_proxy(mesh.proxy, request).await;

        // then: 404 and the page names the missing member.
        assert_eq!(reply.status, 404, "reply: {}", reply.head);
        assert!(reply.body.contains("nobody"), "body: {}", reply.body);
    }
}

// ---- setup and helpers ----

/// Two nodes (A named, with publications; B with a proxy) and B's proxy address.
struct Mesh {
    a: RunningNode,
    b: RunningNode,
    proxy: SocketAddr,
}

/// A is `Café Router` with the publications `published`; B knows A, A's name has reached B, B's proxy is up.
async fn setup(published: Vec<Published>) -> Mesh {
    let (a, b) = pair_named(NAME, published).await;
    let proxy = proxy_of(&b).await;
    Mesh { a, b, proxy }
}

fn all(port: u16, name: &str) -> Published {
    Published {
        proto: Proto::Tcp,
        port,
        name: name.to_string(),
        audience: Audience::All,
    }
}

/// A is named `name` and publishes `published`, B has neither; both are members of each other and B has
/// learned A's name (it arrives with the name observer, every 2 s).
async fn pair_named(name: &str, published: Vec<Published>) -> (RunningNode, RunningNode) {
    let key = NetworkKey::generate().unwrap();
    let mut cfg_a = NodeConfig::for_loopback_test(key.clone());
    cfg_a.settings.exit_enabled = false;
    cfg_a.settings.node_name = Some(name.to_string());
    cfg_a.settings.ports_published = published;
    let mut cfg_b = NodeConfig::for_loopback_test(key);
    cfg_b.settings.exit_enabled = false;
    let a = node::start_node(cfg_a).await.unwrap();
    let b = node::start_node(cfg_b).await.unwrap();
    introduce(&a, &b).await;
    wait_member(&a, b.id, Duration::from_secs(20)).await;
    wait_member(&b, a.id, Duration::from_secs(20)).await;
    wait_for(Duration::from_secs(20), "A's name at B", || {
        b.members.name_of(a.id).as_deref() == Some(name)
    })
    .await;
    (a, b)
}

/// B's proxy address. The front lives until the end of the test process: it is leaked on purpose,
/// the port must stay up while the test talks to it.
async fn proxy_of(node: &RunningNode) -> SocketAddr {
    let front = front_for(node).await;
    let addr = front.addr().expect("the proxy port must be up");
    std::mem::forget(front);
    addr
}

/// Waits until `cond` becomes true, no longer than `limit`.
async fn wait_for(limit: Duration, what: &str, mut cond: impl FnMut() -> bool) {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        if cond() {
            return;
        }
        sleep(Duration::from_millis(100)).await;
    }
    panic!("timed out after {limit:?}: {what}");
}

/// Waits until B reads A's readable entry for `port` from the ports document.
async fn wait_ports_seen(mesh: &Mesh, port: u16) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        let seen = mesh.b.ports_doc.read_all().await.unwrap_or_default();
        if seen
            .iter()
            .any(|p| p.author == mesh.a.id && p.port == port && p.record.is_some())
        {
            return;
        }
        sleep(Duration::from_millis(100)).await;
    }
    panic!("B did not see A's entry for port {port} within 20 s");
}

/// An HTTP/1.1 reply split into status, head and body (the apostrophe entities are folded to `'`).
struct Reply {
    status: u16,
    head: String,
    body: String,
}

fn parse_reply(raw: &[u8]) -> Reply {
    let text = String::from_utf8_lossy(raw).into_owned();
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((text.as_str(), ""));
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or(0);
    let body = body
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&apos;", "'");
    Reply {
        status,
        head: head.to_string(),
        body,
    }
}

/// Absolute-URI `GET` for the address on `port`, as a browser sends to an HTTP proxy.
fn get_through_proxy(port: u16) -> String {
    format!(
        "GET http://{ADDRESS}:{port}/ HTTP/1.1\r\nHost: {ADDRESS}:{port}\r\nConnection: close\r\n\r\n"
    )
}

/// `GET /` in origin form with `Host` set to the address, as a browser sends inside a tunnel.
fn origin_form_request() -> String {
    format!("GET / HTTP/1.1\r\nHost: {ADDRESS}\r\nConnection: close\r\n\r\n")
}

/// Sends `request` to the HTTP proxy and reads the reply until the connection closes.
async fn http_via_proxy(proxy: SocketAddr, request: &str) -> Reply {
    let mut s = TcpStream::connect(proxy).await.unwrap();
    s.write_all(request.as_bytes()).await.unwrap();
    parse_reply(&read_to_close(&mut s).await)
}

/// Sends `CONNECT target`, reads the proxy's reply up to the end of its head (byte by byte, so that nothing
/// after it is taken); returns the stream and the status line.
async fn connect_via(proxy: SocketAddr, target: &str) -> (TcpStream, String) {
    let mut s = TcpStream::connect(proxy).await.unwrap();
    let head = format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n\r\n");
    s.write_all(head.as_bytes()).await.unwrap();
    let mut reply = Vec::new();
    let mut byte = [0u8; 1];
    while !reply.ends_with(b"\r\n\r\n") {
        let n = timeout(Duration::from_secs(10), s.read(&mut byte))
            .await
            .expect("no reply to CONNECT")
            .unwrap();
        assert_eq!(n, 1, "closed before the end of the CONNECT reply");
        reply.push(byte[0]);
    }
    let text = String::from_utf8_lossy(&reply).into_owned();
    let status = text.lines().next().unwrap_or_default().to_string();
    (s, status)
}

async fn read_to_close(s: &mut TcpStream) -> Vec<u8> {
    let mut out = Vec::new();
    timeout(Duration::from_secs(15), s.read_to_end(&mut out))
        .await
        .expect("the connection did not close within 15 s")
        .unwrap();
    out
}

/// The connection is closed without data: EOF (or a reset) and no byte before it.
async fn assert_closed_without_data(s: &mut TcpStream) {
    let mut buf = [0u8; 16];
    let r = timeout(Duration::from_secs(10), s.read(&mut buf))
        .await
        .expect("the connection stayed open for 10 s");
    assert!(matches!(r, Ok(0) | Err(_)), "data arrived: {r:?}");
}
