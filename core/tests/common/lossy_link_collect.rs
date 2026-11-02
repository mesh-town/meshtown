//! Receive collectors of the forwarding check of the `lossy_link` proxy: what arrived, from where, and its size.
//! Plugged in as `#[path = "common/lossy_link_collect.rs"] mod collected;` next to `mod common;` (the rig).

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use tokio::net::UdpSocket;
use tokio::time::timeout;

use super::common::header;

pub struct Arrival {
    pub seq: u32,
    pub len: usize,
    pub from: SocketAddr,
}

/// Reads the socket until nothing arrives for `idle`.
pub async fn collect(sock: &UdpSocket, epoch: Instant, idle: Duration) -> Vec<Arrival> {
    let mut buf = vec![0u8; 65536];
    let mut got = Vec::new();
    while let Ok(Ok((n, from))) = timeout(idle, sock.recv_from(&mut buf)).await {
        let (seq, _) = header(&buf[..n], epoch);
        got.push(Arrival { seq, len: n, from });
    }
    got
}
