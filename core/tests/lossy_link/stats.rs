//! Packet counters of the proxy: one set per direction.
//!
//! What it does: atomic counters written by the proxy tasks, and their snapshot for the test.
//! What it does not have: decisions on a packet's fate — they are in `model.rs`.

use portable_atomic::AtomicU64;
use std::fmt;
use std::sync::atomic::Ordering;

/// A snapshot of one direction's counters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DirStats {
    /// Packets received from the sender.
    pub received: u64,
    /// Packets delivered to the receiver (the counter grows right before sending).
    pub delivered: u64,
    /// Lost by the loss model.
    pub lost_model: u64,
    /// Dropped by the bottleneck queue (tail drop).
    pub dropped_queue: u64,
}

/// A snapshot of both directions: `c2s` is from the client to the target, `s2c` is from the target to the client.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LinkStats {
    pub c2s: DirStats,
    pub s2c: DirStats,
    /// Datagrams eaten by the sink at addresses taken by `block_addr`.
    pub blocked: u64,
}

impl fmt::Display for DirStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "rx={}/tx={}/loss={}/drop={}",
            self.received, self.delivered, self.lost_model, self.dropped_queue
        )
    }
}

impl fmt::Display for LinkStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "c2s[{}] s2c[{}] blocked={}",
            self.c2s, self.s2c, self.blocked
        )
    }
}

/// Live counters of a direction.
#[derive(Default)]
pub struct Counters {
    received: AtomicU64,
    delivered: AtomicU64,
    lost_model: AtomicU64,
    dropped_queue: AtomicU64,
}

impl Counters {
    pub fn received(&self) {
        self.received.fetch_add(1, Ordering::Relaxed);
    }

    pub fn delivered(&self) {
        self.delivered.fetch_add(1, Ordering::Relaxed);
    }

    pub fn lost_model(&self) {
        self.lost_model.fetch_add(1, Ordering::Relaxed);
    }

    pub fn dropped_queue(&self) {
        self.dropped_queue.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> DirStats {
        DirStats {
            received: self.received.load(Ordering::Relaxed),
            delivered: self.delivered.load(Ordering::Relaxed),
            lost_model: self.lost_model.load(Ordering::Relaxed),
            dropped_queue: self.dropped_queue.load(Ordering::Relaxed),
        }
    }
}
