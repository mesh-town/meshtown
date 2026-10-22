//! Integration tests for the HTTP proxy on the SOCKS5 port.
//!
//! What it does: starts real nodes on loopback and goes through their proxy port by three
//! HTTP paths — forwarding a request with an absolute URI, a `CONNECT` tunnel and a refusal — directly and
//! through an exit; checks the head rewriting, the bytes after the head, a head split into chunks,
//! the 400/502 codes and that the same port still serves SOCKS5.
//! Autoconfiguration file: `GET`/`HEAD /proxy.pac` on the same port.
//! What it does not do: does not call an external `curl`, does not go to the internet, does not start TLS.
//! Links: `meshtown::proxy` through `meshtown::node`, the client is `meshtown::http_min`.

#[path = "common/socks_pair.rs"]
mod common;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use meshtown::http_min::{self, HttpLimits, HttpGetRequest, HttpResponse};
use meshtown::network_key::NetworkKey;
use meshtown::node::{self, NodeConfig, RunningNode};
use meshtown::socks5;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{sleep, timeout};

use common::{echo_site, pair_with_static_exit, serve, wait_exit_route, DOMAIN};

const BODY: usize = 4 * 1024;

mod direct_path {
    use super::*;

    mod forwarding {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
        async fn absolute_uri_returns_200_with_full_body() {
            let (node, proxy) = direct_node().await;
            let mut stream = TcpStream::connect(proxy).await.unwrap();

            let response = get(&mut stream, "http://site.test/").await;

            assert_eq!(response.status, 200);
            assert_eq!(response.body.len(), BODY);
            drop(node);
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
        async fn head_in_two_writes_with_pause_still_gets_200() {
            let (node, proxy) = direct_node().await;
            let mut stream = TcpStream::connect(proxy).await.unwrap();

            stream
                .write_all(b"GET http://site.test/ HTTP/1.1\r\nHost: site.test\r\n")
                .await
                .unwrap();
            sleep(Duration::from_millis(50)).await;
            stream.write_all(b"Connection: close\r\n\r\n").await.unwrap();
            let answer = read_all(&mut stream).await;

            assert!(answer.starts_with(b"HTTP/1.1 200"), "reply: {}", lossy(&answer));
            drop(node);
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
        async fn head_is_rewritten_before_reaching_the_server() {
            // given: an echo server and a request with Proxy-Connection and a foreign Host.
            let echo = echo_site().await;
            let (node, proxy) = direct_node().await;
            let mut stream = TcpStream::connect(proxy).await.unwrap();
            let request = format!(
                "GET http://127.0.0.1:{p}/a?b=1 HTTP/1.1\r\nProxy-Connection: keep-alive\r\nHost: x\r\n\r\n",
                p = echo.port()
            );

            // when
            stream.write_all(request.as_bytes()).await.unwrap();
            let seen = lossy(&read_until_head_end(&mut stream).await);

            // then
            assert!(seen.starts_with("GET /a?b=1 HTTP/1.1\r\n"), "echo: {seen:?}");
            assert!(seen.contains(&format!("Host: 127.0.0.1:{}", echo.port())), "echo: {seen:?}");
            assert!(seen.contains("Connection: close"), "echo: {seen:?}");
            assert!(!seen.contains("Proxy-Connection"), "echo: {seen:?}");
            assert!(!seen.contains("Host: x"), "echo: {seen:?}");
            drop(node);
        }
    }

    mod tunnel {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
        async fn connect_then_get_returns_200_with_full_body() {
            let (node, proxy) = direct_node().await;

            let (mut stream, status) = connect_via(proxy, b"CONNECT site.test:80 HTTP/1.1\r\nHost: site.test:80\r\n\r\n").await;
            let response = get(&mut stream, "/").await;

            assert!(status.contains("200"), "CONNECT status: {status:?}");
            assert_eq!(response.status, 200);
            assert_eq!(response.body.len(), BODY);
            drop(node);
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
        async fn bytes_sent_together_with_connect_head_are_not_lost() {
            let (node, proxy) = direct_node().await;
            let mut stream = TcpStream::connect(proxy).await.unwrap();

            stream
                .write_all(
                    b"CONNECT site.test:80 HTTP/1.1\r\n\r\nGET / HTTP/1.1\r\nHost: site.test\r\n\r\n",
                )
                .await
                .unwrap();
            let answer = read_all(&mut stream).await;

            let text = lossy(&answer);
            let established = text.find("200 Connection established").expect("no reply to CONNECT");
            let served = text.find("HTTP/1.1 200 OK").expect("the early request was lost");
            assert!(established < served, "the site's reply came before the reply to CONNECT: {text:?}");
            assert!(answer.len() >= BODY, "the body did not arrive in full: {} bytes", answer.len());
            drop(node);
        }
    }

    mod nothing_listens_on_target {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
        async fn connect_and_forward_both_answer_502() {
            // given: a port whose listener is closed.
            let port = free_port().await;
            let (node, proxy) = direct_node().await;

            // when
            let (_stream, status) =
                connect_via(proxy, format!("CONNECT 127.0.0.1:{port} HTTP/1.1\r\n\r\n").as_bytes()).await;
            let mut stream = TcpStream::connect(proxy).await.unwrap();
            let forwarded = http_min::http_get(
                &mut stream,
                HttpGetRequest {
                    host: "127.0.0.1",
                    path: &format!("http://127.0.0.1:{port}/"),
                    accept: "*/*",
                    user_agent: "meshtown-test",
                },
                HttpLimits::default(),
            )
            .await
            .unwrap();

            // then
            assert!(status.starts_with("HTTP/1.1 502"), "CONNECT: {status:?}");
            assert_eq!(forwarded.status, 502);
            drop(node);
        }
    }
}

mod through_exit {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn forwarding_and_tunnel_carry_traffic_via_chosen_exit() {
        // given: A knows B as its only exit; the route has converged.
        let site = serve(4).await;
        let (a, _b) = pair_with_static_exit(site).await;
        wait_exit_route(&a, Duration::from_secs(20)).await;
        let proxy = a.socks_addr.expect("the proxy port must be up");

        // when/then: forwarding.
        let mut stream = TcpStream::connect(proxy).await.unwrap();
        let forwarded = get(&mut stream, "http://site.test/").await;
        assert_eq!(forwarded.status, 200);
        assert_eq!(forwarded.body.len(), BODY);

        // when/then: the tunnel.
        let (mut tunnel, status) = connect_via(proxy, b"CONNECT site.test:80 HTTP/1.1\r\n\r\n").await;
        let tunneled = get(&mut tunnel, "/").await;
        assert!(status.contains("200"), "CONNECT status: {status:?}");
        assert_eq!(tunneled.status, 200);
        assert_eq!(tunneled.body.len(), BODY);
    }
}

mod same_port {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn socks5_still_works_after_empty_and_http_connections() {
        // given: the connection is closed without bytes and one HTTP connection has been served.
        let (node, proxy) = direct_node().await;
        drop(TcpStream::connect(proxy).await.unwrap());
        let mut http = TcpStream::connect(proxy).await.unwrap();
        assert_eq!(get(&mut http, "http://site.test/").await.status, 200);

        // when
        let mut stream = socks5::client_connect(proxy, DOMAIN, 80).await.unwrap();

        // then
        assert_eq!(get(&mut stream, "/").await.status, 200);
        drop(node);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn origin_form_request_gets_400() {
        let (node, proxy) = direct_node().await;
        let mut stream = TcpStream::connect(proxy).await.unwrap();

        stream.write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n").await.unwrap();
        let answer = read_all(&mut stream).await;

        assert!(answer.starts_with(b"HTTP/1.1 400"), "reply: {}", lossy(&answer));
        drop(node);
    }
}

mod pac_file {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn get_returns_script_with_actual_proxy_port() {
        // given: a node with a proxy port chosen by the OS.
        let (node, proxy) = direct_node().await;

        // when
        let reply = pac_reply_of(proxy, "GET", "/proxy.pac").await;

        // then
        let (head, body) = split_head(&reply);
        assert!(head.starts_with("HTTP/1.1 200"), "reply: {}", lossy(&reply));
        assert!(head.contains("Content-Type: application/x-ns-proxy-autoconfig"), "reply: {}", lossy(&reply));
        let port = proxy.port();
        let route = format!("SOCKS5 127.0.0.1:{port}; PROXY 127.0.0.1:{port}");
        assert!(body.contains(&route), "the body has no path line {route:?}: {body}");
        drop(node);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn head_returns_headers_without_body() {
        let (node, proxy) = direct_node().await;

        let reply = pac_reply_of(proxy, "HEAD", "/proxy.pac").await;

        let (head, body) = split_head(&reply);
        assert!(head.starts_with("HTTP/1.1 200"), "reply: {}", lossy(&reply));
        assert!(head.contains("Content-Length: "), "HEAD has no Content-Length: {head}");
        assert_eq!(body, "", "HEAD has no body");
        drop(node);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn query_tail_still_gets_script() {
        let (node, proxy) = direct_node().await;

        let reply = pac_reply_of(proxy, "GET", "/proxy.pac?x=1").await;

        assert!(reply.starts_with(b"HTTP/1.1 200"), "reply: {}", lossy(&reply));
        drop(node);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn other_paths_still_get_400() {
        let (node, proxy) = direct_node().await;

        let reply = pac_reply_of(proxy, "GET", "/wpad.dat").await;

        assert!(reply.starts_with(b"HTTP/1.1 400"), "reply: {}", lossy(&reply));
        drop(node);
    }
}

/// A node with a direct route: `site.test` → a 4 KiB server. Returns the node (keep it alive) and the proxy address.
async fn direct_node() -> (RunningNode, SocketAddr) {
    let site = serve(4).await;
    let mut cfg = NodeConfig::for_loopback_test(NetworkKey::generate().unwrap());
    cfg.exit_hosts = HashMap::from([(DOMAIN.to_string(), site)]);
    let node = node::start_node(cfg).await.unwrap();
    let proxy = node.socks_addr.expect("the proxy port must be up");
    (node, proxy)
}

/// Writes a raw `CONNECT` head, reads the reply up to `\r\n\r\n` (byte by byte — takes nothing extra);
/// returns the stream and the first line of the reply.
async fn connect_via(proxy: SocketAddr, head: &[u8]) -> (TcpStream, String) {
    let mut stream = TcpStream::connect(proxy).await.unwrap();
    stream.write_all(head).await.unwrap();
    let mut reply = Vec::new();
    let mut byte = [0u8; 1];
    while !reply.ends_with(b"\r\n\r\n") {
        let n = timeout(Duration::from_secs(10), stream.read(&mut byte))
            .await
            .expect("no reply to CONNECT")
            .unwrap();
        assert_eq!(n, 1, "the connection closed before the end of the reply: {}", lossy(&reply));
        reply.push(byte[0]);
    }
    let text = lossy(&reply);
    let status = text.lines().next().unwrap_or_default().to_string();
    (stream, status)
}

/// `GET` with `Host: site.test` and the given `path` (an absolute URI or `/`) over a ready stream.
async fn get(stream: &mut TcpStream, path: &str) -> HttpResponse {
    http_min::http_get(
        stream,
        HttpGetRequest { host: DOMAIN, path, accept: "*/*", user_agent: "meshtown-test" },
        HttpLimits::default(),
    )
    .await
    .unwrap()
}

async fn read_all(stream: &mut TcpStream) -> Vec<u8> {
    let mut out = Vec::new();
    timeout(Duration::from_secs(10), stream.read_to_end(&mut out))
        .await
        .expect("the connection did not close within 10 s")
        .unwrap();
    out
}

/// Sends a `method path` request in origin-form to the proxy port and reads the reply until the connection closes.
async fn pac_reply_of(proxy: SocketAddr, method: &str, path: &str) -> Vec<u8> {
    let mut stream = TcpStream::connect(proxy).await.unwrap();
    let request = format!("{method} {path} HTTP/1.1\r\nHost: {proxy}\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    read_all(&mut stream).await
}

/// Splits the reply into the head (without `\r\n\r\n`) and the body.
fn split_head(reply: &[u8]) -> (String, String) {
    let text = lossy(reply);
    let (head, body) = text.split_once("\r\n\r\n").expect("no end of head in the reply");
    (head.to_string(), body.to_string())
}

async fn read_until_head_end(stream: &mut TcpStream) -> Vec<u8> {
    let mut out = Vec::new();
    let mut buf = [0u8; 1024];
    while !out.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = timeout(Duration::from_secs(10), stream.read(&mut buf))
            .await
            .expect("no echo within 10 s")
            .unwrap();
        assert!(n > 0, "the connection closed before the end of the head: {}", lossy(&out));
        out.extend_from_slice(&buf[..n]);
    }
    out
}

async fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    listener.local_addr().unwrap().port()
}

fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}
