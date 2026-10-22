//! Source of network change events: the network fingerprint and a wrapper over `netwatch`.

use crate::net::stream::BoxFuture;
use netwatch::interfaces::State;
use netwatch::netmon::Monitor;
use n0_watcher::{Direct, Watcher};
use std::net::{IpAddr, Ipv4Addr};
use tokio::time::Instant;

/// What happened to the node's network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NetEvent {
    /// Address, default route or wake-up: there is a network, but a different one.
    Changed,
    /// No usable network is left.
    Lost,
}

/// Event source; `None` — the source ran dry.
pub(crate) trait NetEvents: Send {
    fn next(&mut self) -> BoxFuture<'_, Option<NetEvent>>;
}

/// Network information used to decide whether it "really changed".
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fingerprint {
    addrs: Vec<Ipv4Addr>,
    route: String,
    woke: Option<Instant>,
}

fn usable(addr: &IpAddr) -> Option<Ipv4Addr> {
    match addr {
        IpAddr::V4(a) if !a.is_loopback() && !a.is_link_local() => Some(*a),
        _ => None,
    }
}

/// Network fingerprint; `None` — there is no network (no usable IPv4 address or default route).
pub(crate) fn fingerprint(
    addrs: impl IntoIterator<Item = IpAddr>,
    default_route: Option<&str>,
    woke: Option<Instant>,
) -> Option<Fingerprint> {
    let route = default_route?.to_string();
    let mut addrs: Vec<Ipv4Addr> = addrs.into_iter().filter_map(|a| usable(&a)).collect();
    addrs.sort();
    addrs.dedup();
    if addrs.is_empty() {
        return None;
    }
    Some(Fingerprint { addrs, route, woke })
}

pub(crate) fn net_event_between(prev: &Option<Fingerprint>, next: &Option<Fingerprint>) -> Option<NetEvent> {
    match (prev, next) {
        (Some(_), None) => Some(NetEvent::Lost),
        (None, Some(_)) => Some(NetEvent::Changed),
        (Some(a), Some(b)) if a != b => Some(NetEvent::Changed),
        _ => None,
    }
}

fn fingerprint_of_state(state: &State) -> Option<Fingerprint> {
    let addrs = state
        .interfaces
        .values()
        .filter(|i| i.is_up())
        .flat_map(|i| i.addrs().map(|n| n.addr()).collect::<Vec<_>>());
    fingerprint(addrs, state.default_route_interface.as_deref(), state.last_unsuspend)
}

/// The real source: the `netwatch` interface monitor.
pub(crate) struct NetwatchSource {
    _monitor: Monitor,
    watcher: Direct<State>,
    prev: Option<Fingerprint>,
}

impl NetwatchSource {
    pub(crate) async fn start() -> Result<Self, String> {
        let monitor = Monitor::new().await.map_err(|e| e.to_string())?;
        let mut watcher = monitor.interface_state();
        let prev = fingerprint_of_state(&watcher.get());
        Ok(Self { _monitor: monitor, watcher, prev })
    }

    async fn next_event(&mut self) -> Option<NetEvent> {
        loop {
            self.watcher.updated().await.ok()?;
            let now = fingerprint_of_state(&self.watcher.get());
            let event = net_event_between(&self.prev, &now);
            self.prev = now;
            if event.is_some() {
                return event;
            }
        }
    }
}

impl NetEvents for NetwatchSource {
    fn next(&mut self) -> BoxFuture<'_, Option<NetEvent>> {
        Box::pin(self.next_event())
    }
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
