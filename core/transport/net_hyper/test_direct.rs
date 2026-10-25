//! Helpers for the direct-dial tests: nodes on a loopback cluster, nodes with a dead DHT, addresses
//! that lead to a live, a dead and a foreign UDP socket.

use super::{HyperOpts, HyperTransport};
use crate::transport::net::addr::{AddrHint, PeerAddr};
use crate::transport::net::contract::secret;
use crate::transport::net::error::NetResult;
use crate::transport::net::id::PeerId;
use crate::transport::net::service::{ServiceId, PORT};
use crate::transport::net::transport::{PeerConn, Transport};
use crate::transport::net_hyper::test_cluster::Cluster;
use std::net::{SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{sleep, timeout};

/// The service every test node serves.
pub(crate) const SVC: ServiceId = PORT;

/// A node that is part of the cluster and serves `SVC`; returns once its announcement is made.
pub(crate) async fn serving(cl: &Cluster, seed: u8) -> Arc<HyperTransport> {
    let opts = HyperOpts {
        bootstrap: Some(cl.bootstrap()),
        ..Default::default()
    };
    let node = HyperTransport::bind(secret(seed), opts)
        .await
        .expect("node came up");
    node.serve(&[SVC]);
    node.wait_listen_outcome().await.expect("listen succeeded");
    node
}

/// A node whose only bootstrap address is dead: the DHT finds nobody, only direct dials work.
pub(crate) async fn isolated(seed: u8) -> Arc<HyperTransport> {
    let opts = HyperOpts {
        bootstrap: Some(vec!["127.0.0.1:1".into()]),
        ..Default::default()
    };
    HyperTransport::bind(secret(seed), opts)
        .await
        .expect("isolated node came up")
}

/// The address of `t` as an invite would carry it: its id and a hint at the DHT socket on loopback.
pub(crate) async fn direct_addr(t: &HyperTransport) -> PeerAddr {
    let port = t.local_port().await.expect("DHT socket port");
    PeerAddr {
        id: t.me(),
        hints: vec![AddrHint::Ip(SocketAddr::from(([127, 0, 0, 1], port)))],
    }
}

/// An address of `id` whose hint points at a UDP port nobody listens on.
pub(crate) fn dead_addr(id: PeerId) -> PeerAddr {
    let closed = {
        let sock = UdpSocket::bind("127.0.0.1:0").expect("free UDP port");
        local_v4(&sock)
    };
    PeerAddr {
        id,
        hints: vec![AddrHint::Ip(SocketAddr::V4(closed))],
    }
}

/// A bound UDP socket that never answers (a foreign protocol); keep it alive while the address is used.
pub(crate) fn silent_socket() -> (UdpSocket, SocketAddrV4) {
    let sock = UdpSocket::bind("127.0.0.1:0").expect("UDP socket");
    let at = local_v4(&sock);
    (sock, at)
}

fn local_v4(sock: &UdpSocket) -> SocketAddrV4 {
    match sock.local_addr().expect("socket address") {
        SocketAddr::V4(at) => at,
        SocketAddr::V6(at) => panic!("expected an IPv4 socket, got {at}"),
    }
}

/// The first `Ip` hint of `addr`.
pub(crate) fn ip_of(addr: &PeerAddr) -> SocketAddr {
    addr.hints
        .iter()
        .find_map(|h| match h {
            AddrHint::Ip(sa) => Some(*sa),
            _ => None,
        })
        .expect("the address has an Ip hint")
}

/// `from.connect(to, SVC)` under a time limit; a miss is a test failure, not a hang.
pub(crate) async fn connect_within(
    from: &HyperTransport,
    to: &PeerAddr,
    limit: Duration,
) -> NetResult<Arc<dyn PeerConn>> {
    timeout(limit, from.connect(to, SVC))
        .await
        .expect("connect did not finish in time")
}

/// Waits up to `limit` until `ready` is true; tells whether it became true.
pub(crate) async fn becomes(limit: Duration, mut ready: impl FnMut() -> bool) -> bool {
    let started = std::time::Instant::now();
    while !ready() {
        if started.elapsed() > limit {
            return false;
        }
        sleep(Duration::from_millis(25)).await;
    }
    true
}
