//! Helpers for `UDP ASSOCIATE` scenarios: a UDP echo server, the SOCKS5 handshake of an association and
//! the datagram exchange with the relay. Plugged in as `#[path = "common/socks_udp.rs"] mod udp;` next to
//! `common/socks_pair.rs`.
#![allow(dead_code)]

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use meshtown_core::proxy::socks5::{self, Target, UdpPacket};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::time::timeout;

/// The external deadline of a network step.
pub(crate) const UDP_LIMIT: Duration = Duration::from_secs(10);

/// A UDP echo server on the given loopback address (`127.0.0.1` or `::1`); `None` if the address cannot be bound.
pub(crate) async fn spawn_udp_echo(ip: IpAddr) -> Option<SocketAddr> {
    let sock = UdpSocket::bind((ip, 0)).await.ok()?;
    let addr = sock.local_addr().ok()?;
    meshtown_core::node::output::spawn(async move {
        let mut buf = vec![0u8; 65_535];
        while let Ok((n, from)) = sock.recv_from(&mut buf).await {
            let _ = sock.send_to(&buf[..n], from).await;
        }
    });
    Some(addr)
}

/// What the proxy answered to `UDP ASSOCIATE`: the control connection, the code and the bound address.
pub(crate) struct Associated {
    pub control: TcpStream,
    pub reply_code: u8,
    pub bound: SocketAddr,
}

/// Greets the proxy and asks for a `UDP ASSOCIATE`; returns whatever the reply was.
pub(crate) async fn request_associate(proxy: SocketAddr) -> Associated {
    let mut control = TcpStream::connect(proxy).await.unwrap();
    control
        .write_all(&[socks5::VERSION, 1, socks5::METHOD_NO_AUTH])
        .await
        .unwrap();
    let mut answer = [0u8; 2];
    control.read_exact(&mut answer).await.unwrap();
    control
        .write_all(&socks5::encode_udp_associate_request())
        .await
        .unwrap();
    let mut reply = [0u8; 10];
    timeout(UDP_LIMIT, control.read_exact(&mut reply))
        .await
        .expect("no reply in time")
        .unwrap();
    let ip = Ipv4Addr::new(reply[4], reply[5], reply[6], reply[7]);
    let bound = SocketAddr::new(IpAddr::V4(ip), u16::from_be_bytes([reply[8], reply[9]]));
    Associated {
        control,
        reply_code: reply[1],
        bound,
    }
}

/// A successful association and a client datagram socket on the loopback.
pub(crate) async fn associate(proxy: SocketAddr) -> (TcpStream, UdpSocket, SocketAddr) {
    let associated = request_associate(proxy).await;
    assert_eq!(
        associated.reply_code,
        socks5::REP_OK,
        "the association must be granted"
    );
    let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    (associated.control, client, associated.bound)
}

/// Sends `data` for `target:port` to the relay.
pub(crate) async fn send_to_relay(
    client: &UdpSocket,
    relay: SocketAddr,
    target: &Target,
    port: u16,
    data: &[u8],
) {
    let bytes = socks5::encode_udp_request(target, port, data).unwrap();
    client.send_to(&bytes, relay).await.unwrap();
}

/// The next datagram the relay sends to `client`, if any arrives within `wait`.
pub(crate) async fn reply_within(client: &UdpSocket, wait: Duration) -> Option<UdpPacket> {
    let mut buf = vec![0u8; 65_535];
    let (n, _) = timeout(wait, client.recv_from(&mut buf)).await.ok()?.ok()?;
    Some(socks5::parse_udp_datagram(&buf[..n]).expect("a valid SOCKS5 UDP datagram"))
}
