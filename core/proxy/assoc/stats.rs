//! Counters and activity shared by the pumps of one association.

use std::net::SocketAddr;
use std::sync::Mutex;
use std::time::Instant;

/// What a relay did: bytes of payload each way and the datagrams it dropped, by reason.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct RelayStats {
    pub up_bytes: u64,
    pub down_bytes: u64,
    pub foreign: u64,
    pub fragmented: u64,
    pub malformed: u64,
    pub oversize: u64,
    pub unroutable: u64,
}

impl RelayStats {
    /// One line about the dropped datagrams, `None` if there were none.
    pub fn drops_line(&self) -> Option<String> {
        let reasons = [
            ("foreign source", self.foreign),
            ("fragmented", self.fragmented),
            ("malformed", self.malformed),
            ("oversize", self.oversize),
            ("unroutable", self.unroutable),
        ];
        let parts: Vec<String> = reasons.iter().filter(|(_, n)| *n > 0).map(|(what, n)| format!("{n} {what}")).collect();
        let total: u64 = reasons.iter().map(|(_, n)| n).sum();
        (total > 0).then(|| format!("udp associate: dropped {total} datagrams ({})", parts.join(", ")))
    }
}

/// State both pumps touch. Locks are held for one assignment, never across an await.
pub(super) struct Shared {
    stats: Mutex<RelayStats>,
    last_activity: Mutex<Instant>,
    client: Mutex<Option<SocketAddr>>,
}

impl Shared {
    pub fn new() -> Self {
        Self { stats: Mutex::default(), last_activity: Mutex::new(Instant::now()), client: Mutex::new(None) }
    }

    pub fn count(&self, change: impl FnOnce(&mut RelayStats)) {
        change(&mut self.stats.lock().unwrap());
    }

    pub fn stats(&self) -> RelayStats {
        self.stats.lock().unwrap().clone()
    }

    pub fn touch(&self) {
        *self.last_activity.lock().unwrap() = Instant::now();
    }

    pub fn last_activity(&self) -> Instant {
        *self.last_activity.lock().unwrap()
    }

    /// The pinned address of the client socket: set by the first accepted datagram.
    pub fn client(&self) -> Option<SocketAddr> {
        *self.client.lock().unwrap()
    }

    pub fn pin_client(&self, addr: SocketAddr) {
        *self.client.lock().unwrap() = Some(addr);
    }
}
