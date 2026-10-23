//! The choice of the network the proxy serves: the listener's target is the published configuration of the
//! chosen network, or nothing while that network is down.

use std::collections::HashMap;
use std::sync::Mutex;

use tokio::sync::watch;

use crate::config::config_file::NetSlotId;
use crate::proxy::ProxyConfig;

use super::{ProxyFace, ProxyFront};

struct SeatState {
    chosen: NetSlotId,
    live: HashMap<NetSlotId, ProxyConfig>,
}

/// Connects networks to the listener: remembers the published configuration of every running network and
/// points the listener at the chosen one. The target is changed under the seat's lock.
pub struct ProxySeat {
    front: Option<ProxyFront>,
    state: Mutex<SeatState>,
}

impl ProxySeat {
    pub fn new(front: Option<ProxyFront>, chosen: NetSlotId) -> Self {
        ProxySeat {
            front,
            state: Mutex::new(SeatState {
                chosen,
                live: HashMap::new(),
            }),
        }
    }

    /// The network `id` is up with this configuration; it becomes the target if it is the chosen one.
    pub fn node_up(&self, id: &NetSlotId, cfg: ProxyConfig) {
        let mut state = self.lock();
        state.live.insert(id.clone(), cfg);
        self.publish(&state, id);
    }

    /// The network `id` is going down; if it is the chosen one, the target becomes empty.
    pub fn node_down(&self, id: &NetSlotId) {
        let mut state = self.lock();
        state.live.remove(id);
        self.publish(&state, id);
    }

    /// Chooses the network: the target is its published configuration, or nothing if it is not running.
    pub fn choose(&self, id: NetSlotId) {
        let mut state = self.lock();
        state.chosen = id.clone();
        self.publish(&state, &id);
    }

    pub fn chosen(&self) -> NetSlotId {
        self.lock().chosen.clone()
    }

    pub fn front(&self) -> Option<ProxyFront> {
        self.front.clone()
    }

    /// The listener's face; without a listener — a constant `OFF`.
    pub fn face(&self) -> watch::Receiver<ProxyFace> {
        match &self.front {
            Some(front) => front.face(),
            None => watch::channel(ProxyFace::OFF).1,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, SeatState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Sets the target if `changed` is the chosen network.
    fn publish(&self, state: &SeatState, changed: &NetSlotId) {
        let Some(front) = self.front.as_ref().filter(|_| *changed == state.chosen) else {
            return;
        };
        front.set_target(state.live.get(changed).cloned());
    }
}

#[cfg(test)]
#[path = "seat_tests.rs"]
mod tests;
