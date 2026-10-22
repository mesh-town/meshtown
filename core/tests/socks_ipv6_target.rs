//! IPv6 targets through the proxy: a SOCKS5 `CONNECT` with address type 4 and an HTTP `CONNECT [v6]:port`
//! reach the exit as a plain literal and the exit opens a connection to it.
//!
//! What it does: two real nodes on loopback (`common::pair_with`); the exit knows the literal `LITERAL` in its
//! `exit_hosts` table and leads it to an echo server, so no IPv6 connectivity is needed. The route of the first
//! node leads through the exit.
//! What it does not do: does not open a real IPv6 connection and does not resolve AAAA records.
//! Links: `meshtown::socks5`, `meshtown::proxy`; the harness is `core/tests/common/socks_pair.rs`.

#[path = "common/socks_pair.rs"]
mod common;

use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use meshtown::route::Route;
use meshtown::socks5::{self, Target};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

use common::{echo_site, pair_with, wait_exit_route};

/// The IPv6 literal the exit maps to the echo server.
const LITERAL: &str = "2001:db8::1";
/// The external deadline of a network step.
const NET_LIMIT: Duration = Duration::from_secs(10);

struct Fixture {
    _nodes: (meshtown::node::RunningNode, meshtown::node::RunningNode),
    proxy: SocketAddr,
}

async fn setup() -> Fixture {
    let site = echo_site().await;
    let (a, b) = pair_with(site, true, |cfg| {
        cfg.exit_hosts.insert(LITERAL.to_string(), site);
    })
    .await;
    a.set_route_exits(vec![b.id]);
    let route = wait_exit_route(&a, Duration::from_secs(20)).await;
    assert_eq!(
        route,
        Route::Exits(vec![b.id]),
        "the route must lead through the exit"
    );
    let proxy = a.socks_addr.expect("the SOCKS5 server must be up");
    Fixture {
        _nodes: (a, b),
        proxy,
    }
}

/// Sends `payload` over an open tunnel and checks that the echo returns it.
async fn assert_echo(stream: &mut TcpStream, payload: &[u8]) {
    stream.write_all(payload).await.unwrap();
    let mut back = vec![0u8; payload.len()];
    timeout(NET_LIMIT, stream.read_exact(&mut back))
        .await
        .expect("the echo did not answer in time")
        .unwrap();
    assert_eq!(back, payload);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn socks_connect_with_ipv6_address_type_reaches_the_target() {
    // given: a route through the exit, which knows the literal
    let fx = setup().await;
    let mut stream = TcpStream::connect(fx.proxy).await.unwrap();
    stream
        .write_all(&[socks5::VERSION, 1, socks5::METHOD_NO_AUTH])
        .await
        .unwrap();
    let mut answer = [0u8; 2];
    stream.read_exact(&mut answer).await.unwrap();

    // when: CONNECT with ATYP 4
    let ip: Ipv6Addr = LITERAL.parse().unwrap();
    let request = socks5::encode_socks5_request(&Target::Ip(IpAddr::V6(ip)), 80).unwrap();
    assert_eq!(request[3], socks5::ATYP_IPV6);
    stream.write_all(&request).await.unwrap();
    let mut reply = [0u8; 10];
    timeout(NET_LIMIT, stream.read_exact(&mut reply))
        .await
        .expect("no reply in time")
        .unwrap();

    // then: success and the echo returns the bytes
    assert_eq!(reply[1], socks5::REP_OK);
    assert_echo(&mut stream, b"hello over ipv6").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn http_connect_to_bracketed_literal_reaches_the_target() {
    // given: the same route
    let fx = setup().await;
    let mut stream = TcpStream::connect(fx.proxy).await.unwrap();

    // when: an HTTP CONNECT with a bracketed literal
    let head = format!("CONNECT [{LITERAL}]:80 HTTP/1.1\r\nHost: [{LITERAL}]:80\r\n\r\n");
    stream.write_all(head.as_bytes()).await.unwrap();
    let ok = b"HTTP/1.1 200 Connection established\r\n\r\n";
    let mut reply = vec![0u8; ok.len()];
    timeout(NET_LIMIT, stream.read_exact(&mut reply))
        .await
        .expect("no reply in time")
        .unwrap();

    // then: the tunnel is up and the echo returns the bytes
    assert_eq!(reply, ok);
    assert_echo(&mut stream, b"hello over http").await;
}
