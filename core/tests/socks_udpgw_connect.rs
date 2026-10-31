//! UDP gateway frames over SOCKS5 `CONNECT` to `240.0.0.1:7300`, end to end: several local connections to one
//! exit each keep their own `conn_id` numbering while the node shares an exit stream between them.
//!
//! What it does: two real nodes on loopback (`common::pair_with`); the exit maps `UDP_NAME` to a local UDP echo
//! server. Frames are written as bytes, the way a gateway client does.
//! Links: `meshtown_core::proxy::socks5`; the harnesses are `core/tests/common/socks_pair.rs` and
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
use tokio::net::TcpStream;
use tokio::time::timeout;

use common::{echo_site, front_for, pair_with, wait_exit_route};
use udp::{spawn_udp_echo, UDP_LIMIT};

/// The name the exit maps to the UDP echo server.
const UDP_NAME: &str = "udp.test";
const FLAG_DATA: u8 = 0x02;
const FLAG_CLOSE: u8 = 0x04;

struct Fixture {
    _nodes: (RunningNode, RunningNode),
    proxy: SocketAddr,
    _front: meshtown_core::proxy::ProxyFront,
    echo_port: u16,
}

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
    let front = front_for(&a).await;
    let proxy = front.addr().expect("the SOCKS5 server must be up");
    Fixture {
        _front: front,
        _nodes: (a, b),
        proxy,
        echo_port: echo.port(),
    }
}

/// A SOCKS5 `CONNECT` to the gateway target.
async fn gateway_connect(proxy: SocketAddr) -> TcpStream {
    let mut stream = TcpStream::connect(proxy).await.unwrap();
    stream
        .write_all(&[socks5::VERSION, 1, socks5::METHOD_NO_AUTH])
        .await
        .unwrap();
    let mut answer = [0u8; 2];
    stream.read_exact(&mut answer).await.unwrap();
    let target = Target::Ip(IpAddr::from([240, 0, 0, 1]));
    let request = socks5::encode_socks5_request(&target, 7300).unwrap();
    stream.write_all(&request).await.unwrap();
    let mut reply = [0u8; 10];
    timeout(UDP_LIMIT, stream.read_exact(&mut reply))
        .await
        .expect("no reply in time")
        .unwrap();
    assert_eq!(reply[1], socks5::REP_OK);
    stream
}

/// A gateway frame with a domain target: `LEN FLAGS CONN_ID ATYP=3 NAME_LEN NAME PORT DATA`.
fn data_frame(conn_id: u16, port: u16, payload: &[u8]) -> Vec<u8> {
    let mut body = vec![FLAG_DATA];
    body.extend(conn_id.to_be_bytes());
    body.push(3);
    body.push(UDP_NAME.len() as u8);
    body.extend(UDP_NAME.as_bytes());
    body.extend(port.to_be_bytes());
    body.extend(payload);
    let mut frame = (body.len() as u16).to_be_bytes().to_vec();
    frame.extend(body);
    frame
}

fn close_frame(conn_id: u16) -> Vec<u8> {
    let mut frame = vec![0, 3, FLAG_CLOSE];
    frame.extend(conn_id.to_be_bytes());
    frame
}

/// The next frame: its flags, `conn_id` and the bytes after the header.
async fn next_frame(stream: &mut TcpStream) -> (u8, u16, Vec<u8>) {
    let mut len = [0u8; 2];
    timeout(UDP_LIMIT, stream.read_exact(&mut len))
        .await
        .expect("no frame in time")
        .unwrap();
    let mut body = vec![0u8; usize::from(u16::from_be_bytes(len))];
    stream.read_exact(&mut body).await.unwrap();
    (
        body[0],
        u16::from_be_bytes([body[1], body[2]]),
        body[3..].to_vec(),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_udpgw_connections_get_their_own_echo() {
    // given: two connections to the gateway target
    let fx = setup().await;
    let mut first = gateway_connect(fx.proxy).await;
    let mut second = gateway_connect(fx.proxy).await;

    // when: both send DATA with the same conn_id and different payloads
    first
        .write_all(&data_frame(1, fx.echo_port, b"from-first"))
        .await
        .unwrap();
    second
        .write_all(&data_frame(1, fx.echo_port, b"from-second"))
        .await
        .unwrap();

    // then: each gets its own payload back under conn_id 1
    for (stream, payload) in [
        (&mut first, &b"from-first"[..]),
        (&mut second, &b"from-second"[..]),
    ] {
        let (flags, conn_id, rest) = next_frame(stream).await;
        assert_eq!((flags, conn_id), (FLAG_DATA, 1));
        assert!(rest.ends_with(payload), "{rest:?}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn close_then_new_id_on_the_same_connection_works() {
    // given: a connection that used conn_id 1
    let fx = setup().await;
    let mut stream = gateway_connect(fx.proxy).await;
    stream
        .write_all(&data_frame(1, fx.echo_port, b"one"))
        .await
        .unwrap();
    assert_eq!(next_frame(&mut stream).await.1, 1);

    // when: it closes id 1 and sends DATA under id 2
    stream.write_all(&close_frame(1)).await.unwrap();
    stream
        .write_all(&data_frame(2, fx.echo_port, b"two"))
        .await
        .unwrap();

    // then: the echo comes back with id 2
    let (flags, conn_id, rest) = next_frame(&mut stream).await;
    assert_eq!((flags, conn_id), (FLAG_DATA, 2));
    assert!(rest.ends_with(b"two"));
}
