//! The UDP echo server of the `UDP ASSOCIATE` scenarios. Plugged in as `#[path = "common/socks_udp.rs"] mod udp;`
//! next to `common/socks_pair.rs`. The deadline lives in `socks_udp_limit.rs`, the association handshake and the
//! datagram exchange in `socks_udp_assoc.rs`.

use std::net::{IpAddr, SocketAddr};

use tokio::net::UdpSocket;

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
