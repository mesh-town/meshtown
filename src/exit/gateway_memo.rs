//! Remembers exits that refused the UDP gateway target, so the node does not ask them again for every session.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::net::PeerId;

/// How long a refusal of the gateway target is trusted before the exit is asked again.
pub const GATEWAY_REFUSAL_TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Clone, Default)]
pub struct GatewayMemo {
    refused: Arc<Mutex<HashMap<PeerId, Instant>>>,
}

impl GatewayMemo {
    /// Notes that `peer` refused the gateway target at `now`; a repeated note extends the window.
    pub fn remember_refused(&self, peer: PeerId, now: Instant) {
        self.refused.lock().expect("gateway memo").insert(peer, now);
    }

    /// Whether `peer` refused the gateway target less than `GATEWAY_REFUSAL_TTL` before `now`.
    pub fn refused_recently(&self, peer: PeerId, now: Instant) -> bool {
        let map = self.refused.lock().expect("gateway memo");
        map.get(&peer).is_some_and(|at| now.saturating_duration_since(*at) < GATEWAY_REFUSAL_TTL)
    }
}

#[cfg(test)]
#[path = "gateway_memo_tests.rs"]
mod tests;
