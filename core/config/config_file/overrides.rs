//! Command-line flags on top of the settings file (docs/CONFIG-FILE.md section 5.1, `CliOverrides::apply`).

use std::net::SocketAddr;

use super::settings::{ListenAccess, Settings, TransportMode};

/// What was typed on the command line. `Some` — the flag was given, `None` — it was not.
/// `Debug` is hand-written, `network_key` prints as `<hidden>` (like `Settings`).
#[derive(Clone, Default)]
pub struct CliOverrides {
    pub network_key: Option<String>,           // --key / MESHTOWN_KEY
    pub transport_mode: Option<TransportMode>, // --relay-only / --no-relay
    pub exit_enabled: Option<bool>,            // --exit / --no-exit
    pub socks_enabled: Option<bool>,           // --socks / --no-socks
    pub socks_port: Option<u16>,               // --socks-port
    pub socks_access: Option<ListenAccess>,    // --socks-access
    pub checks_enabled: Option<bool>,          // --checks / --no-checks
    pub check_interval_sec: Option<u64>,       // --check-interval
    pub doh_server: Option<SocketAddr>,        // --check-doh
    pub provider_dns: Option<SocketAddr>,      // --provider-dns
    pub probe_interval_sec: Option<u64>,       // --probe-interval
    pub probe_full_cycle: Option<bool>,        // --probe-full-cycle
    pub web_port: Option<u16>,                 // --web-port
}

impl std::fmt::Debug for CliOverrides {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CliOverrides")
            .field(
                "network_key",
                &self.network_key.as_ref().map(|_| "<hidden>"),
            )
            .field("transport_mode", &self.transport_mode)
            .field("exit_enabled", &self.exit_enabled)
            .field("socks_enabled", &self.socks_enabled)
            .field("socks_port", &self.socks_port)
            .field("socks_access", &self.socks_access)
            .field("checks_enabled", &self.checks_enabled)
            .field("check_interval_sec", &self.check_interval_sec)
            .field("doh_server", &self.doh_server)
            .field("provider_dns", &self.provider_dns)
            .field("probe_interval_sec", &self.probe_interval_sec)
            .field("probe_full_cycle", &self.probe_full_cycle)
            .field("web_port", &self.web_port)
            .finish()
    }
}

impl CliOverrides {
    /// Every flag except the network key: the key of an added network comes from its own file.
    pub fn for_added_slot(&self) -> CliOverrides {
        CliOverrides {
            network_key: None,
            ..self.clone()
        }
    }

    /// Puts what was typed on top of `s`. Returns the paths of the overridden fields in the order of the docs/CONFIG-FILE.md table,
    /// section 3: "network.key", "transport.mode",
    /// "exit.enabled", "socks.enabled", "socks.port", "socks.access", "checks.enabled", "checks.interval_sec",
    /// "checks.doh_server", "checks.provider_dns", "probe.interval_sec", "web.port".
    /// A path goes into the list if the flag was given, even when the value matches the file.
    pub fn apply(&self, s: &mut Settings) -> Vec<&'static str> {
        let mut overridden_paths = Vec::new();
        self.apply_transport(s, &mut overridden_paths);
        self.apply_service(s, &mut overridden_paths);
        overridden_paths
    }

    /// The first half of the table: network and transport.
    fn apply_transport(&self, s: &mut Settings, overridden_paths: &mut Vec<&'static str>) {
        if let Some(v) = &self.network_key {
            s.net.network_key = Some(v.clone());
            overridden_paths.push("network.key");
        }
        if let Some(v) = self.transport_mode {
            s.net.transport_mode = v;
            overridden_paths.push("transport.mode");
        }
    }

    /// The second half of the table: exit/socks/checks/probe/web.
    fn apply_service(&self, s: &mut Settings, overridden_paths: &mut Vec<&'static str>) {
        if let Some(v) = self.exit_enabled {
            s.net.exit_enabled = v;
            overridden_paths.push("exit.enabled");
        }
        if let Some(v) = self.socks_enabled {
            s.process.socks_enabled = v;
            overridden_paths.push("socks.enabled");
        }
        if let Some(v) = self.socks_port {
            s.process.socks_port = v;
            overridden_paths.push("socks.port");
        }
        if let Some(v) = self.socks_access {
            s.process.socks_access = v;
            overridden_paths.push("socks.access");
        }
        if let Some(v) = self.checks_enabled {
            s.net.checks_enabled = v;
            overridden_paths.push("checks.enabled");
        }
        if let Some(v) = self.check_interval_sec {
            s.net.check_interval_sec = v;
            overridden_paths.push("checks.interval_sec");
        }
        if let Some(v) = self.doh_server {
            s.net.doh_server = v;
            overridden_paths.push("checks.doh_server");
        }
        if let Some(v) = self.provider_dns {
            s.net.provider_dns = Some(v);
            overridden_paths.push("checks.provider_dns");
        }
        if let Some(v) = self.probe_interval_sec {
            s.net.probe_interval_sec = v;
            overridden_paths.push("probe.interval_sec");
        }
        if let Some(v) = self.probe_full_cycle {
            s.net.probe_full_cycle = v;
            overridden_paths.push("probe.full_cycle");
        }
        if let Some(v) = self.web_port {
            s.process.web_port = v;
            overridden_paths.push("web.port");
        }
    }
}

#[cfg(test)]
#[path = "overrides_tests.rs"]
mod tests;
