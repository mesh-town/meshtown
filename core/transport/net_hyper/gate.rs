//! Peer admission on the server: open (default) or by a "peer, service" predicate.
//! The policy is set by the node, the adapter does not know it; the check runs in `accept` after the service is chosen.

use crate::transport::net::id::PeerId;
use crate::transport::net::service::ServiceId;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, RwLock};

/// Admission predicate: `true` = let the peer into the service.
pub type Admit = Arc<dyn Fn(PeerId, &ServiceId) -> bool + Send + Sync>;

pub struct PeerGate {
    admit: RwLock<Option<Admit>>,
}

impl PeerGate {
    /// Lets everyone in until a predicate is set.
    pub fn open() -> Self {
        Self {
            admit: RwLock::new(None),
        }
    }

    /// `Some(predicate)` — it decides; `None` — everyone again.
    pub fn set_admit(&self, admit: Option<Admit>) {
        *self.admit.write().unwrap_or_else(|e| e.into_inner()) = admit;
    }

    /// Whether to let the peer into the service; a predicate panic means refusal.
    pub fn admits(&self, peer: PeerId, svc: &ServiceId) -> bool {
        let admit = self.admit.read().unwrap_or_else(|e| e.into_inner()).clone();
        let Some(admit) = admit else { return true };
        catch_unwind(AssertUnwindSafe(|| admit(peer, svc))).unwrap_or(false)
    }
}

#[cfg(test)]
#[path = "gate_tests.rs"]
mod tests;
