//! The shared harness for the `lossy_link` proxy self-check: timestamped packets and the
//! "client → proxy → target" rig.
//!
//! Plugged in from `lossy_link_forwarding.rs` and `lossy_link_shape.rs` via
//! `#[path = "common/lossy_link.rs"] mod common;` next to `mod lossy_link;`. What only one of them needs lives in
//! `lossy_link_collect.rs` (forwarding) and `lossy_link_timed.rs` (shape); the loss-burst counter shared with the
//! model check is `lossy_link_gaps.rs`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::lossy_link::{LinkConfig, LossyLink};
use tokio::net::UdpSocket;

pub const HEADER: usize = 12;

/// A packet with a number and a send timestamp (microseconds from `epoch`), padded to `size` bytes.
pub fn packet(seq: u32, size: usize, epoch: Instant) -> Vec<u8> {
    let mut buf = vec![0u8; size.max(HEADER)];
    buf[..4].copy_from_slice(&seq.to_le_bytes());
    let micros = epoch.elapsed().as_micros() as u64;
    buf[4..HEADER].copy_from_slice(&micros.to_le_bytes());
    buf
}

/// The number and the one-way delay of a received packet.
pub fn header(buf: &[u8], epoch: Instant) -> (u32, Duration) {
    let seq = u32::from_le_bytes(buf[..4].try_into().unwrap());
    let sent = u64::from_le_bytes(buf[4..HEADER].try_into().unwrap());
    let delay = epoch.elapsed().saturating_sub(Duration::from_micros(sent));
    (seq, delay)
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
}
