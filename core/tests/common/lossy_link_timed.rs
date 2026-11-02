//! Timed receive collectors and the burst sender of the shape check of the `lossy_link` proxy: every arrival
//! keeps its one-way delay and its receive moment.
//! Plugged in as `#[path = "common/lossy_link_timed.rs"] mod timed;` next to `mod common;` (the rig).

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::net::UdpSocket;
use tokio::time::{sleep, timeout};

use super::common::{header, Rig};

pub struct Arrival {
    pub seq: u32,
    pub len: usize,
    pub delay: Duration,
    pub at: Instant,
    pub from: SocketAddr,
}

/// Reads the socket until nothing arrives for `idle`.
pub async fn collect(sock: &UdpSocket, epoch: Instant, idle: Duration) -> Vec<Arrival> {
    let mut buf = vec![0u8; 65536];
    let mut got = Vec::new();
    while let Ok(Ok((n, from))) = timeout(idle, sock.recv_from(&mut buf)).await {
        let (seq, delay) = header(&buf[..n], epoch);
        got.push(Arrival {
            seq,
            len: n,
            delay,
            at: Instant::now(),
            from,
        });
    }
    got
}

/// Starts `collect` in advance: the arrival time is recorded at the moment of receipt, not when the test
/// gets to the socket.
pub fn spawn_collect(
    sock: &Arc<UdpSocket>,
    epoch: Instant,
    idle: Duration,
) -> tokio::task::JoinHandle<Vec<Arrival>> {
    let sock = sock.clone();
    meshtown_core::node::output::spawn(async move { collect(&sock, epoch, idle).await })
}

impl Rig {
    /// Sends `n` packets with the client, occasionally yielding so the socket buffer does not overflow.
    pub async fn burst_c2s(&self, n: u32, size: usize) {
        for seq in 0..n {
            self.send_c2s(seq, size).await;
            if seq % 100 == 99 {
                sleep(Duration::from_millis(1)).await;
            }
        }
    }
}
