//! The shared harness for the `lossy_link` proxy self-check: timestamped packets, receive collectors,
//! the "client → proxy → target" rig and loss-burst counting.
//!
//! Plugged in from `lossy_link_forwarding.rs`, `lossy_link_shape.rs`, `lossy_link_model.rs` via
//! `#[path = "common/lossy_link.rs"] mod common;` next to `mod lossy_link;`.
#![allow(dead_code)] // each test crate takes its own subset

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::lossy_link::{LinkConfig, LossyLink};
use tokio::net::UdpSocket;
use tokio::time::{sleep, timeout};

pub const HEADER: usize = 12;

/// A packet with a number and a send timestamp (microseconds from `epoch`), padded to `size` bytes.
pub fn packet(seq: u32, size: usize, epoch: Instant) -> Vec<u8> {
    let mut buf = vec![0u8; size.max(HEADER)];
    buf[..4].copy_from_slice(&seq.to_le_bytes());
    let micros = epoch.elapsed().as_micros() as u64;
    buf[4..HEADER].copy_from_slice(&micros.to_le_bytes());
    buf
}

pub struct Arrival {
    pub seq: u32,
    pub len: usize,
    pub delay: Duration,
    pub at: Instant,
    pub from: SocketAddr,
}

pub fn parse(buf: &[u8], from: SocketAddr, epoch: Instant) -> Arrival {
    let seq = u32::from_le_bytes(buf[..4].try_into().unwrap());
    let sent = u64::from_le_bytes(buf[4..HEADER].try_into().unwrap());
    let delay = epoch.elapsed().saturating_sub(Duration::from_micros(sent));
    Arrival {
        seq,
        len: buf.len(),
        delay,
        at: Instant::now(),
        from,
    }
}

/// Reads the socket until nothing arrives for `idle`.
pub async fn collect(sock: &UdpSocket, epoch: Instant, idle: Duration) -> Vec<Arrival> {
    let mut buf = vec![0u8; 65536];
    let mut got = Vec::new();
    while let Ok(Ok((n, from))) = timeout(idle, sock.recv_from(&mut buf)).await {
        got.push(parse(&buf[..n], from, epoch));
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
    tokio::spawn(async move { collect(&sock, epoch, idle).await })
}

pub struct Rig {
    pub link: LossyLink,
    pub target: Arc<UdpSocket>,
    pub client: Arc<UdpSocket>,
    pub epoch: Instant,
}

pub async fn rig(edit: impl FnOnce(&mut LinkConfig)) -> Rig {
    let target = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());
    let mut cfg = LinkConfig::plain(target.local_addr().unwrap());
    edit(&mut cfg);
    let link = LossyLink::start(cfg);
    let client = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());
    Rig {
        link,
        target,
        client,
        epoch: Instant::now(),
    }
}

impl Rig {
    pub async fn send_c2s(&self, seq: u32, size: usize) {
        let pkt = packet(seq, size, self.epoch);
        self.client
            .send_to(&pkt, self.link.front_addr())
            .await
            .unwrap();
    }

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

pub fn gaps(seqs: &[u32], total: u32) -> Vec<u32> {
    let mut seen = vec![false; total as usize];
    seqs.iter().for_each(|s| seen[*s as usize] = true);
    let mut runs = Vec::new();
    let mut run = 0;
    for ok in seen {
        match ok {
            false => run += 1,
            true if run > 0 => runs.push(std::mem::take(&mut run)),
            true => {}
        }
    }
    if run > 0 {
        runs.push(run);
    }
    runs
}

pub fn model_cfg(edit: impl FnOnce(&mut LinkConfig)) -> LinkConfig {
    let mut cfg = LinkConfig::plain("127.0.0.1:9".parse().unwrap());
    edit(&mut cfg);
    cfg
}

pub async fn collect_raw(sock: &UdpSocket, buf: &mut [u8]) -> Vec<Vec<u8>> {
    let mut got = Vec::new();
    while let Ok(Ok((n, _))) = timeout(Duration::from_millis(300), sock.recv_from(buf)).await {
        got.push(buf[..n].to_vec());
    }
    got
}
