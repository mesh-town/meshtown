//! Relay side of the transport: where candidates come from, the dial memory and the relay node of this member.

use super::HyperTransport;
use crate::transport::net::id::PeerId;
use crate::transport::net_hyper::relay::host::{StartRelay, RELAY_MODE_POLL};
use crate::transport::net_hyper::relay::{
    relay_opts, spawn_host, RelayAdmit, RelayBook, RelayCandidate, RelayHost, RelaySource,
};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::{MutexGuard, RwLock};
use tokio::sync::watch;
use townjump_dht::{Dht, NodeMode};
use townjump_dht_peeroxide::RelayOpts;

/// Dial-side relay state: the candidate source and the memory of which relay worked.
#[derive(Default)]
pub(super) struct RelayDial {
    source: RwLock<Option<RelaySource>>,
    pub(super) book: RelayBook,
    #[cfg(test)]
    pub(super) faults: std::sync::Mutex<DialFaults>,
    #[cfg(test)]
    pub(super) direct_runs: std::sync::atomic::AtomicUsize,
}

impl RelayDial {
    pub(super) fn set_source(&self, source: Option<RelaySource>) {
        *self.source.write().unwrap_or_else(|e| e.into_inner()) = source;
    }

    #[cfg(test)]
    pub(super) fn direct_fails(&self) -> bool {
        self.faults
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .direct_fails
    }

    #[cfg(not(test))]
    pub(super) fn direct_fails(&self) -> bool {
        false
    }

    #[cfg(test)]
    pub(super) fn relay_fails(&self, key: &[u8; 32]) -> bool {
        let faults = self.faults.lock().unwrap_or_else(|e| e.into_inner());
        faults.failing_relays.contains(key)
    }

    #[cfg(not(test))]
    pub(super) fn relay_fails(&self, _key: &[u8; 32]) -> bool {
        false
    }

    #[cfg(test)]
    pub(super) fn count_direct_run(&self) {
        self.direct_runs.fetch_add(1, Ordering::SeqCst);
    }

    #[cfg(not(test))]
    pub(super) fn count_direct_run(&self) {}

    pub(super) fn has_source(&self) -> bool {
        self.source
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
    }

    /// Candidates for a dial to `to`; no source — none. The source runs outside the lock.
    pub(super) async fn candidates(&self, to: PeerId) -> Vec<RelayCandidate> {
        let source = self
            .source
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        match source {
            Some(source) => source(to).await,
            None => Vec::new(),
        }
    }
}

/// Failures injected into dials by tests.
#[cfg(test)]
#[derive(Clone, Default)]
pub(crate) struct DialFaults {
    /// The direct phase fails at once.
    pub direct_fails: bool,
    /// Relays (by key) whose phase fails at once.
    pub failing_relays: Vec<[u8; 32]>,
}

impl HyperTransport {
    /// `Some(source)` — dials may go through a relay of a member the source names; `None` — never.
    pub fn set_relay_source(&self, source: Option<RelaySource>) {
        self.dialer.relay.set_source(source);
    }

    /// Starts the relay node of this member (once) and returns its announced key:
    /// `Some` while the relay is announced, `None` otherwise. After `close` nothing starts.
    /// Only peers `admit` accepts may pair through the relay; a second call keeps the first `admit`.
    pub fn serve_relay(&self, admit: RelayAdmit) -> watch::Receiver<Option<[u8; 32]>> {
        let mut slot = self.host_slot();
        if let Some(host) = slot.as_ref() {
            return host.keys();
        }
        if self.close_started.load(Ordering::SeqCst) {
            return watch::channel(None).1;
        }
        let host = self.spawn_relay_host(admit);
        let keys = host.keys();
        *slot = Some(host);
        keys
    }

    pub(super) fn host_slot(&self) -> MutexGuard<'_, Option<RelayHost>> {
        self.relay_host.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Options of the relay node: the permanent relay key and the admission check.
    pub(super) fn relay_opts_for(&self, admit: RelayAdmit) -> RelayOpts {
        relay_opts(self.relay_identity.clone(), admit)
    }

    fn spawn_relay_host(&self, admit: RelayAdmit) -> RelayHost {
        let node = self.node.clone();
        let mode: Arc<dyn Fn() -> NodeMode + Send + Sync> = Arc::new(move || node.node_mode());
        let factory = self.relay_factory.clone();
        let opts = self.relay_opts_for(admit);
        let start: StartRelay = Arc::new(move || {
            let factory = factory.clone();
            let opts = opts.clone();
            Box::pin(async move { factory.create_relay_with(opts).await })
        });
        spawn_host(mode, start, RELAY_MODE_POLL)
    }

    #[cfg(test)]
    pub(crate) fn set_dial_faults_for_tests(&self, faults: DialFaults) {
        *self
            .dialer
            .relay
            .faults
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = faults;
    }

    #[cfg(test)]
    pub(crate) fn relay_source_is_set(&self) -> bool {
        self.dialer.relay.has_source()
    }

    /// How many times the direct phase ran for a dial that had a relay candidate.
    #[cfg(test)]
    pub(crate) fn direct_phase_runs_for_tests(&self) -> usize {
        self.dialer.relay.direct_runs.load(Ordering::SeqCst)
    }
}
