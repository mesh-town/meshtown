//! Flooding settings: the hop count, timeouts, queue and cache sizes.
//! There is no logic; the defaults come from the design.

use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Tuning {
    /// Hops of a new message.
    pub ttl_hops: u8,
    /// How long without a write before a `Ping` is sent.
    pub keepalive: Duration,
    /// Silence after which a peer counts as gone.
    pub idle: Duration,
    /// The handshake timeout.
    pub handshake: Duration,
    /// How many ids the duplicate cache holds.
    pub seen_cap: usize,
    /// The lifetime of an entry in the duplicate cache.
    pub seen_life: Duration,
    pub peer_write_queue_len: usize,
    pub event_channel_len: usize,
}

impl Default for Tuning {
    fn default() -> Self {
        Tuning {
            ttl_hops: 6,
            keepalive: Duration::from_secs(10),
            idle: Duration::from_secs(30),
            handshake: Duration::from_secs(5),
            seen_cap: 4096,
            seen_life: Duration::from_secs(300),
            peer_write_queue_len: 256,
            event_channel_len: 1024,
        }
    }
}
