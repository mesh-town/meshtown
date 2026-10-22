//! The direct upstream of an association: datagrams leave the node itself, as a direct TCP connection does.
//!
//! One socket per address family; the IPv6 one is optional, and without it IPv6 datagrams are dropped. No
//! address guard of an exit applies here: a local network and the loopback are reachable.

use std::future::pending;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket as StdUdpSocket};
use std::sync::Arc;

use tokio::net::UdpSocket;

use crate::proxy::socks5::{Target, UdpPacket};
use crate::site_checks::doh::is_fake_ip;

use super::super::path::address_of;
use super::super::serve::ProxyConfig;
use super::relay::RECV_BUFFER;
use super::upstream::{Closed, Delivery, UpstreamRx, UpstreamTx};

/// The sockets of a direct association.
struct Sockets {
    v4: UdpSocket,
    v6: Option<UdpSocket>,
}

pub(super) struct DirectTx {
    cfg: ProxyConfig,
    sockets: Arc<Sockets>,
}

pub(super) struct DirectRx {
    sockets: Arc<Sockets>,
}

/// Opens the sockets; fails only if the IPv4 one cannot be bound.
pub(super) fn open_direct(cfg: &ProxyConfig) -> io::Result<(DirectTx, DirectRx)> {
    let v4 = bind(IpAddr::V4(Ipv4Addr::UNSPECIFIED))?;
    let v6 = bind(IpAddr::V6(Ipv6Addr::UNSPECIFIED)).ok();
    let sockets = Arc::new(Sockets { v4, v6 });
    Ok((
        DirectTx {
            cfg: cfg.clone(),
            sockets: sockets.clone(),
        },
        DirectRx { sockets },
    ))
}

fn bind(ip: IpAddr) -> io::Result<UdpSocket> {
    let std_socket = StdUdpSocket::bind((ip, 0))?;
    std_socket.set_nonblocking(true)?;
    UdpSocket::from_std(std_socket)
}

impl UpstreamTx for DirectTx {
    async fn send(&mut self, packet: UdpPacket) -> Result<Delivery, Closed> {
        let Some(addr) = self.destination(&packet.target, packet.port).await else {
            return Ok(Delivery::Unroutable);
        };
        let Some(socket) = self.sockets.for_address(addr) else {
            return Ok(Delivery::Unroutable);
        };
        let sent = socket.send_to(&packet.data, addr).await;
        Ok(if sent.is_ok() {
            Delivery::Sent
        } else {
            Delivery::Unroutable
        })
    }
}

impl DirectTx {
    /// Where the datagram goes: the target as is (an IPv4-mapped address as IPv4), unless it is closed.
    async fn destination(&self, target: &Target, port: u16) -> Option<SocketAddr> {
        let addr = address_of(&self.cfg, target, port).await.ok()?;
        let addr = SocketAddr::new(addr.ip().to_canonical(), addr.port());
        (!is_closed_target(addr)).then_some(addr)
    }
}

impl Sockets {
    fn for_address(&self, addr: SocketAddr) -> Option<&UdpSocket> {
        if addr.is_ipv4() {
            return Some(&self.v4);
        }
        self.v6.as_ref()
    }
}

/// Targets no datagram is sent to: port 0, "any" and multicast addresses, the IPv4 broadcast and the fake-ip pool.
fn is_closed_target(addr: SocketAddr) -> bool {
    let ip = addr.ip();
    let closed_v4 = matches!(ip, IpAddr::V4(v4) if v4.is_broadcast() || is_fake_ip(v4));
    addr.port() == 0 || ip.is_unspecified() || ip.is_multicast() || closed_v4
}

impl UpstreamRx for DirectRx {
    async fn recv(&mut self) -> Option<(SocketAddr, Vec<u8>)> {
        let mut v4_buf = vec![0u8; RECV_BUFFER];
        let mut v6_buf = vec![0u8; RECV_BUFFER];
        loop {
            let received = tokio::select! {
                got = self.sockets.v4.recv_from(&mut v4_buf) => got.map(|(n, from)| (from, v4_buf[..n].to_vec())),
                got = recv_v6(self.sockets.v6.as_ref(), &mut v6_buf) => got.map(|(n, from)| (from, v6_buf[..n].to_vec())),
            };
            match received {
                Ok(reply) => return Some(reply),
                Err(e) if is_peer_error(&e) => continue,
                Err(_) => return None,
            }
        }
    }
}

async fn recv_v6(socket: Option<&UdpSocket>, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
    let Some(socket) = socket else {
        return pending().await;
    };
    socket.recv_from(buf).await
}

/// An error caused by the other end of an earlier send (some systems report it on the next read): not the end.
fn is_peer_error(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionRefused
    )
}

#[cfg(test)]
#[path = "direct_tests.rs"]
mod tests;
