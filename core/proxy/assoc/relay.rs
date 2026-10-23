//! The relay of one association: client datagrams to the upstream and replies back, until the control
//! connection closes, the upstream closes, the stop signal fires or the association is idle.

use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::net::{TcpStream, UdpSocket};

use crate::proxy::mesh::host::MeshHost;
use crate::proxy::socks5::{encode_udp_reply, parse_udp_datagram, UdpHeaderError};

use super::stats::{RelayStats, Shared};
use super::upstream::{Closed, Delivery, UpstreamRx, UpstreamTx};

/// The UDP datagram buffer: the largest datagram there can be.
pub(super) const RECV_BUFFER: usize = 65_535;

pub(super) struct RelayLimits {
    /// How long an association may carry no datagram in either direction.
    pub idle: Duration,
}

/// Runs the association to its end and returns what it did. The control connection is closed on return.
pub(super) async fn relay(
    control: TcpStream,
    udp: UdpSocket,
    client_ip: IpAddr,
    tx: impl UpstreamTx,
    rx: impl UpstreamRx,
    stop: impl Future<Output = ()> + Send,
    limits: RelayLimits,
) -> RelayStats {
    let shared = Shared::new();
    tokio::select! {
        () = pump_up(&udp, client_ip, &shared, tx) => {}
        () = pump_down(&udp, &shared, rx) => {}
        () = control_closed(control) => {}
        () = stop => {}
        () = idle(&shared, limits.idle) => {}
    }
    shared.stats()
}

/// Client socket to upstream.
async fn pump_up(udp: &UdpSocket, client_ip: IpAddr, shared: &Shared, mut tx: impl UpstreamTx) {
    let mut buf = vec![0u8; RECV_BUFFER];
    loop {
        let Ok((n, source)) = udp.recv_from(&mut buf).await else {
            return;
        };
        if !from_client(source, client_ip, shared) {
            shared.count(|s| s.foreign += 1);
            continue;
        }
        let packet = match parse_udp_datagram(&buf[..n]) {
            Ok(packet) => packet,
            Err(UdpHeaderError::Fragmented(_)) => {
                shared.count(|s| s.fragmented += 1);
                continue;
            }
            Err(_) => {
                shared.count(|s| s.malformed += 1);
                continue;
            }
        };
        if MeshHost::parse(&packet.target).is_some() {
            // Mesh hosts are answered by the node over TCP only; a datagram never reaches an exit or DNS.
            shared.count(|s| s.unroutable += 1);
            continue;
        }
        shared.pin_client(source);
        shared.touch();
        let len = packet.data.len() as u64;
        match tx.send(packet).await {
            Ok(delivery) => record(shared, delivery, len),
            Err(Closed) => return,
        }
    }
}

/// Whether the datagram comes from the client: its IP, and once a port is pinned, that port.
fn from_client(source: SocketAddr, client_ip: IpAddr, shared: &Shared) -> bool {
    let pinned = shared
        .client()
        .is_none_or(|pinned| pinned.port() == source.port());
    pinned && source.ip().to_canonical() == client_ip.to_canonical()
}

fn record(shared: &Shared, delivery: Delivery, len: u64) {
    match delivery {
        Delivery::Sent => shared.count(|s| s.up_bytes += len),
        Delivery::Oversize => shared.count(|s| s.oversize += 1),
        Delivery::Unroutable => shared.count(|s| s.unroutable += 1),
    }
}

/// Upstream to client socket. A reply before the first client datagram has nowhere to go and is dropped;
/// a failed send is a skipped datagram, not the end.
async fn pump_down(udp: &UdpSocket, shared: &Shared, mut rx: impl UpstreamRx) {
    while let Some((from, data)) = rx.recv().await {
        let Some(client) = shared.client() else {
            continue;
        };
        shared.touch();
        if udp
            .send_to(&encode_udp_reply(from, &data), client)
            .await
            .is_ok()
        {
            shared.count(|s| s.down_bytes += data.len() as u64);
        }
    }
}

/// Completes when the client closes the control connection (anything it writes is ignored).
async fn control_closed(mut control: TcpStream) {
    let mut sink = [0u8; 64];
    while matches!(control.read(&mut sink).await, Ok(n) if n > 0) {}
}

/// Completes after `limit` without a datagram in either direction.
async fn idle(shared: &Shared, limit: Duration) {
    loop {
        let deadline = shared.last_activity() + limit;
        tokio::time::sleep_until(deadline.into()).await;
        if shared.last_activity() + limit <= std::time::Instant::now() {
            return;
        }
    }
}

#[cfg(test)]
#[path = "relay_tests.rs"]
mod tests;
