//! SOCKS5 `UDP ASSOCIATE` end to end: a client datagram socket, the proxy of the first node, the UDP gateway
//! of an exit and a UDP echo server.
//!
//! What it does: two real nodes on loopback (`common::pair_with`); the exit maps the name `UDP_NAME` to a local
//! UDP echo server, so no outside network is needed. The route of the first node leads through the exit. Checks the
//! datagram round trip, the filter on the source of client datagrams, the end of the association with its
//! control connection, and that `CONNECT` keeps working beside an association.
//! What it does not do: does not reach real addresses (the exit closes loopback and private ranges).
//! Links: `meshtown_core::proxy::socks5`, `meshtown_core::proxy`; the harnesses are `core/tests/common/socks_pair.rs` and
//! `core/tests/common/socks_udp.rs`.

#[path = "common/socks_pair.rs"]
mod common;
#[path = "common/socks_udp.rs"]
mod udp;

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use meshtown_core::node::RunningNode;
use meshtown_core::proxy::route::Route;
use meshtown_core::proxy::socks5::{self, Target};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::time::{sleep, timeout, Instant};

use common::{echo_site, pair_with, wait_exit_route, DOMAIN};
use udp::{associate, reply_within, send_to_relay, spawn_udp_echo, UDP_LIMIT};

/// The name the exit maps to the UDP echo server.
const UDP_NAME: &str = "udp.test";
/// How long to wait for a reply that must not come.
const SILENCE: Duration = Duration::from_millis(500);

struct Fixture {
    _nodes: (RunningNode, RunningNode),
    proxy: SocketAddr,
    echo: SocketAddr,
}

/// Two nodes with the route of the first one through the exit.
async fn setup() -> Fixture {
    let echo = spawn_udp_echo(IpAddr::from([127, 0, 0, 1]))
        .await
        .expect("the loopback is up");
    let site = echo_site().await;
    let (a, b) = pair_with(site, true, |cfg| {
        cfg.exit_hosts.insert(UDP_NAME.to_string(), echo);
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
        echo,
    }
}

fn name() -> Target {
    Target::Domain(UDP_NAME.to_string())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn datagram_goes_through_exit_and_back() {
    // given: an association through the exit
    let fx = setup().await;
    let (_control, client, bound) = associate(fx.proxy).await;
    assert_eq!(bound.ip(), IpAddr::from([127, 0, 0, 1]));
    assert_ne!(bound.port(), 0);

    // when: two datagrams for a name that the exit maps to the echo server
    for payload in [&b"first"[..], &b"second"[..]] {
        send_to_relay(&client, bound, &name(), fx.echo.port(), payload).await;

        // then: each comes back with the source of the echo server
        let reply = reply_within(&client, UDP_LIMIT)
            .await
            .expect("an echo in time");
        assert_eq!(reply.data, payload);
        assert_eq!(reply.target, Target::Ip(fx.echo.ip()));
        assert_eq!(reply.port, fx.echo.port());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn datagram_from_another_port_is_ignored() {
    // given: an association whose client port is pinned by a first datagram
    let fx = setup().await;
    let (_control, client, bound) = associate(fx.proxy).await;
    send_to_relay(&client, bound, &name(), fx.echo.port(), b"mine").await;
    assert!(reply_within(&client, UDP_LIMIT).await.is_some());

    // when: another socket of the same host sends through the relay
    let intruder = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    send_to_relay(&intruder, bound, &name(), fx.echo.port(), b"theirs").await;

    // then: it gets nothing, and the real client is still served
    assert!(reply_within(&intruder, SILENCE).await.is_none());
    send_to_relay(&client, bound, &name(), fx.echo.port(), b"again").await;
    assert_eq!(
        reply_within(&client, UDP_LIMIT)
            .await
            .expect("an echo in time")
            .data,
        b"again"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn closing_the_control_connection_closes_the_relay() {
    // given: a working association
    let fx = setup().await;
    let (control, client, bound) = associate(fx.proxy).await;
    send_to_relay(&client, bound, &name(), fx.echo.port(), b"hello").await;
    assert!(reply_within(&client, UDP_LIMIT).await.is_some());

    // when: the client closes the control connection
    drop(control);

    // then: the relay socket goes away, so a datagram to it is refused by the host
    client.connect(bound).await.unwrap();
    let deadline = Instant::now() + UDP_LIMIT;
    let mut buf = [0u8; 64];
    loop {
        let _ = client.send(b"probe").await;
        match timeout(Duration::from_millis(200), client.recv(&mut buf)).await {
            Ok(Err(_)) => return,
            _ => assert!(Instant::now() < deadline, "the relay socket stayed open"),
        }
        sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn connect_still_works_beside_an_association() {
    // given: an open association
    let fx = setup().await;
    let (_control, _client, _bound) = associate(fx.proxy).await;

    // when: a CONNECT through the same proxy to a name the exit maps to the TCP echo server
    let mut stream = TcpStream::connect(fx.proxy).await.unwrap();
    stream
        .write_all(&[socks5::VERSION, 1, socks5::METHOD_NO_AUTH])
        .await
        .unwrap();
    let mut answer = [0u8; 2];
    stream.read_exact(&mut answer).await.unwrap();
    let request = socks5::encode_socks5_request(&Target::Domain(DOMAIN.to_string()), 80).unwrap();
    stream.write_all(&request).await.unwrap();
    let mut reply = [0u8; 10];
    timeout(UDP_LIMIT, stream.read_exact(&mut reply))
        .await
        .expect("no reply in time")
        .unwrap();

    // then: the tunnel works
    assert_eq!(reply[1], socks5::REP_OK);
    stream.write_all(b"ping").await.unwrap();
    let mut back = [0u8; 4];
    timeout(UDP_LIMIT, stream.read_exact(&mut back))
        .await
        .expect("no echo in time")
        .unwrap();
    assert_eq!(&back, b"ping");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn associate_without_exits_relays_directly() {
    // given: a node whose route has no exits, and a UDP echo server on the loopback
    let echo = spawn_udp_echo(IpAddr::from([127, 0, 0, 1]))
        .await
        .expect("the loopback is up");
    let site = echo_site().await;
    let (a, _b) = pair_with(site, true, |_| {}).await;
    let proxy = a.socks_addr.expect("the SOCKS5 server must be up");
    let (_control, client, bound) = associate(proxy).await;

    // when: a datagram for the echo server
    send_to_relay(
        &client,
        bound,
        &Target::Ip(echo.ip()),
        echo.port(),
        b"direct",
    )
    .await;

    // then: the reply comes from the node itself, with the source of the echo server
    let reply = reply_within(&client, UDP_LIMIT)
        .await
        .expect("an echo in time");
    assert_eq!(reply.data, b"direct");
    assert_eq!(
        (reply.target, reply.port),
        (Target::Ip(echo.ip()), echo.port())
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn direct_association_reaches_an_ipv6_target() {
    // given: no exits and an IPv6 echo server (the test ends early on a host without IPv6 loopback)
    let Some(echo) = spawn_udp_echo(IpAddr::from([0, 0, 0, 0, 0, 0, 0, 1])).await else {
        eprintln!("no IPv6 loopback on this host: skipped");
        return;
    };
    let site = echo_site().await;
    let (a, _b) = pair_with(site, true, |_| {}).await;
    let proxy = a.socks_addr.expect("the SOCKS5 server must be up");
    let (_control, client, bound) = associate(proxy).await;

    // when: a datagram with an IPv6 address
    send_to_relay(&client, bound, &Target::Ip(echo.ip()), echo.port(), b"six").await;

    // then: the echo comes back with the IPv6 source
    let reply = reply_within(&client, UDP_LIMIT)
        .await
        .expect("an echo in time");
    assert_eq!(
        (reply.data.as_slice(), reply.target, reply.port),
        (&b"six"[..], Target::Ip(echo.ip()), echo.port())
    );
}
