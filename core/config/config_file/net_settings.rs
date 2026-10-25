//! The network part of the settings (`NetSettings`): everything the running node reads (`NodeConfig.settings`).
//! What isn't here: process fields (`process_settings.rs`) and JSON parsing (`parse.rs`).

use std::net::SocketAddr;
use std::time::Duration;

use crate::ports_sharing::types::{Opened, Published};
use crate::transport::net::{NetChoice, PeerId};

use super::settings::TransportMode;

/// The network fields of the `config` document, all filled in. Defaults live only in `Default`.
/// `Debug` is hand-written, without the network key: otherwise `{:?}` of `NodeConfig` would print it in full.
#[derive(Clone, PartialEq)]
pub struct NetSettings {
    pub network_key: Option<String>, // network.key       — only in the file
    pub node_name: Option<String>, // node.name         — the node's display name, read at start by `pick_node_name`
    pub transport_mode: TransportMode, // transport.mode
    pub exit_enabled: bool,        // exit.enabled
    pub route_exits: Vec<PeerId>,  // route.exits
    pub ports_published: Vec<Published>, // ports.published
    pub ports_opened: Vec<Opened>, // ports.opened
    pub checks_enabled: bool,      // checks.enabled
    pub check_interval_sec: u64,   // checks.interval_sec
    pub doh_server: SocketAddr,    // checks.doh_server
    pub provider_dns: Option<SocketAddr>, // checks.provider_dns
    pub geo_markers: Vec<String>,  // checks.geo_markers — extra region-stub phrases, lowercase
    pub probe_interval_sec: u64,   // probe.interval_sec
    pub probe_full_cycle: bool,    // probe.full_cycle — 1 MiB to all peers, not only to the exit
    pub net_choice: NetChoice,     // net.{transports,discovery,sync} — choice of the network stack
}

/// A phone does not serve as an exit unless the user turns it on; every other platform does.
pub(crate) fn default_exit_enabled(android: bool) -> bool {
    !android
}

impl Default for NetSettings {
    /// Defaults per `docs/CONFIG-FILE.md`, section 3, and `docs/WEB-API-config.md`, section 2.
    fn default() -> Self {
        NetSettings {
            network_key: None,
            node_name: None,
            transport_mode: TransportMode::DirectAndRelay,
            exit_enabled: default_exit_enabled(cfg!(target_os = "android")),
            route_exits: vec![],
            ports_published: vec![],
            ports_opened: vec![],
            checks_enabled: true,
            check_interval_sec: 900,
            doh_server: crate::site_checks::doh::DEFAULT_DOH,
            provider_dns: None,
            geo_markers: vec![],
            probe_interval_sec: 300,
            probe_full_cycle: false,
            net_choice: crate::node::net_names::default_choice(),
        }
    }
}

impl std::fmt::Debug for NetSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let key = self.network_key.as_ref().map(|_| "<hidden>");
        f.debug_struct("NetSettings")
            .field("network_key", &key)
            .field("node_name", &self.node_name)
            .field("transport_mode", &self.transport_mode)
            .field("exit_enabled", &self.exit_enabled)
            .field("route_exits", &self.route_exits)
            .field("ports_published", &self.ports_published)
            .field("ports_opened", &self.ports_opened)
            .field("checks_enabled", &self.checks_enabled)
            .field("check_interval_sec", &self.check_interval_sec)
            .field("doh_server", &self.doh_server)
            .field("provider_dns", &self.provider_dns)
            .field("geo_markers", &self.geo_markers)
            .field("probe_interval_sec", &self.probe_interval_sec)
            .field("probe_full_cycle", &self.probe_full_cycle)
            .field("net_choice", &self.net_choice)
            .finish()
    }
}

impl NetSettings {
    /// Whether the node goes through the relay: `DirectAndRelay` and `RelayOnly`.
    pub fn uses_relay(&self) -> bool {
        self.transport_mode != TransportMode::NoRelay
    }

    /// Whether it goes through the relay only: `RelayOnly`.
    pub fn is_relay_only(&self) -> bool {
        self.transport_mode == TransportMode::RelayOnly
    }

    /// The bandwidth probe interval.
    pub fn probe_interval(&self) -> Duration {
        Duration::from_secs(self.probe_interval_sec)
    }

    /// The base interval of a domain check.
    pub fn check_interval(&self) -> Duration {
        Duration::from_secs(self.check_interval_sec)
    }
}

#[cfg(test)]
#[path = "net_settings_tests.rs"]
mod tests;
