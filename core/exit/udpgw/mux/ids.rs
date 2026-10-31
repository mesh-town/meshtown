//! The bindings of one shared stream: which local connection and local id stand behind each shared id.
//! Pure bookkeeping, no networking.

use std::collections::HashMap;
use std::time::Duration;

use tokio::time::Instant;

/// Bindings per shared stream. Equal to the exit's session limit: beyond it the exit drops new sessions.
pub(super) const LANE_SESSIONS: usize = 256;
/// A binding without traffic in either direction lives this long. Equal to the exit's session idle time.
pub(super) const SESSION_IDLE: Duration = Duration::from_secs(60);

/// Identifies one local connection inside a shared stream.
pub(super) type LocalKey = u64;

struct Binding {
    shared: u16,
    last: Instant,
}

pub(super) struct SessionTable {
    cap: usize,
    /// The next shared id to try. Ids go round: a released id comes back only after a full turn, so a late
    /// reply of a closed session cannot reach a new owner of the number.
    next: u16,
    by_local: HashMap<(LocalKey, u16), Binding>,
    by_shared: HashMap<u16, (LocalKey, u16)>,
}

fn step(id: u16) -> u16 {
    if id == u16::MAX {
        return 1;
    }
    id + 1
}

impl SessionTable {
    pub(super) fn new(cap: usize) -> Self {
        SessionTable {
            cap: cap.min(usize::from(u16::MAX) - 1),
            next: 1,
            by_local: HashMap::new(),
            by_shared: HashMap::new(),
        }
    }

    pub(super) fn len(&self) -> usize {
        self.by_local.len()
    }

    pub(super) fn has(&self, local: LocalKey, local_id: u16) -> bool {
        self.by_local.contains_key(&(local, local_id))
    }

    /// The shared id of the binding, created if new; `None` when the table is full.
    pub(super) fn bind(&mut self, local: LocalKey, local_id: u16, now: Instant) -> Option<u16> {
        if let Some(binding) = self.by_local.get_mut(&(local, local_id)) {
            binding.last = now;
            return Some(binding.shared);
        }
        if self.by_local.len() >= self.cap {
            return None;
        }
        let shared = self.issue();
        self.by_local
            .insert((local, local_id), Binding { shared, last: now });
        self.by_shared.insert(shared, (local, local_id));
        Some(shared)
    }

    fn issue(&mut self) -> u16 {
        let mut id = self.next;
        while self.by_shared.contains_key(&id) {
            id = step(id);
        }
        self.next = step(id);
        id
    }

    /// Removes the binding and returns its shared id.
    pub(super) fn unbind(&mut self, local: LocalKey, local_id: u16) -> Option<u16> {
        let binding = self.by_local.remove(&(local, local_id))?;
        self.by_shared.remove(&binding.shared);
        Some(binding.shared)
    }

    /// Who a reply with this shared id is for; counts as traffic.
    pub(super) fn route_reply(&mut self, shared: u16, now: Instant) -> Option<(LocalKey, u16)> {
        let owner = *self.by_shared.get(&shared)?;
        if let Some(binding) = self.by_local.get_mut(&owner) {
            binding.last = now;
        }
        Some(owner)
    }

    /// Removes every binding of the local connection and returns their shared ids.
    pub(super) fn unbind_local(&mut self, local: LocalKey) -> Vec<u16> {
        let ids: Vec<u16> = self
            .by_local
            .keys()
            .filter(|(key, _)| *key == local)
            .map(|(_, id)| *id)
            .collect();
        ids.into_iter()
            .filter_map(|id| self.unbind(local, id))
            .collect()
    }

    /// Removes the bindings without traffic for `idle` and returns their shared ids.
    pub(super) fn sweep(&mut self, now: Instant, idle: Duration) -> Vec<u16> {
        let stale: Vec<(LocalKey, u16)> = self
            .by_local
            .iter()
            .filter(|(_, b)| now.saturating_duration_since(b.last) >= idle)
            .map(|(owner, _)| *owner)
            .collect();
        stale
            .into_iter()
            .filter_map(|(local, id)| self.unbind(local, id))
            .collect()
    }
}

#[cfg(test)]
#[path = "ids_tests.rs"]
mod tests;
