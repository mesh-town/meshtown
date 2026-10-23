//! Node settings for assembly (`NodeConfig`): the preset for the CLI and the preset for loopback tests.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use crate::config::config_file::{NetSettings, TransportMode};
use crate::transport::net::{NodeSecret, PeerAddr};

use crate::members::network_key::NetworkKey;

use super::identity::{IdentityState, TemporaryReason};

/// The node's SOCKS listener (from `socks.*` of the settings file).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SocksListen {
    pub enabled: bool,
    /// Port 0 — any free one (tests).
    pub port: u16,
    /// Who may connect: `localhost` — listen on `127.0.0.1`, `localnet` — on `0.0.0.0`.
    pub access: crate::config::config_file::ListenAccess,
    /// Over which networks to accept (`socks.ifaces`); `Auto` — automatic.
    pub ifaces: crate::config::config_file::IfaceFilter,
}

impl SocksListen {
    /// The listener is off.
    pub const OFF: SocksListen = SocksListen {
        enabled: false,
        port: 0,
        access: crate::config::config_file::ListenAccess::Localhost,
        ifaces: crate::config::config_file::IfaceFilter::Auto,
    };
}

/// Node settings for assembly.
#[derive(Clone, Debug)]
pub struct NodeConfig {
    pub network_key: NetworkKey,
    /// The network part of the settings file; the node reads it only from here.
    pub settings: NetSettings,
    /// SOCKS listener: the process part of the settings that the node still holds.
    pub socks: SocksListen,
    pub probe_first_delay: Duration,
    /// Domains the node adds to its list at startup.
    pub initial_domains: Vec<String>,
    /// Whether to ask external sources for our own country and announce it to peers.
    pub geoip: bool,
    /// The proxy is off by the `--no-socks` flag, not by a setting; the log line needs this.
    pub socks_off_by_flag: bool,
    /// The published state of the proxy listener when it lives outside the node; `None` — the node's own.
    pub proxy_face: Option<tokio::sync::watch::Receiver<crate::proxy::ProxyFace>>,
    /// Domains the node treats as blocked on its side (the `--block-domain` flag).
    pub blocked_domains: Vec<String>,
    /// Substitute domain addresses for the exit: tests only, always empty in the node.
    pub exit_hosts: HashMap<String, SocketAddr>,
    /// The node secret from the settings file. `None` — a new one on every start (tests, `--no-config`).
    pub identity_secret: Option<NodeSecret>,
    /// Whether the node identity is kept between starts.
    pub identity: IdentityState,
    /// Member addresses from the invite (`--invite`), entered at startup as discovered.
    pub initial_peers: Vec<PeerAddr>,
    /// The network label from the invite or the settings file.
    pub network_label: Option<String>,
    /// File of the peer address cache; `None` — the cache is off (tests, `--no-config`).
    pub peers_cache_path: Option<PathBuf>,
    /// Directory of the document and blob store (the domain list); `None` — in memory (tests,
    /// `--no-config`).
    pub docs_store_dir: Option<PathBuf>,
    /// File of the last known member names; `None` — neither read nor written (tests,
    /// `--no-config`).
    pub peer_names_path: Option<PathBuf>,
    /// The `mem` network in process memory: tests only. `Some` allows the `mem` transport in `net.transports`
    /// (it can't be chosen from the settings file), `None` — a live node.
    #[doc(hidden)]
    pub test_net: Option<std::sync::Arc<crate::transport::net::mem::MemNet>>,
    /// Test knob: the membership list is not consulted, every peer that passes the handshake is let in;
    /// set by the loopback preset.
    #[doc(hidden)]
    pub open_list: bool,
}

impl NodeConfig {
    /// Node settings for the CLI: `settings` are cloned, the rest are the live node's defaults.
    /// The live path goes through `Settings::node_config`, here SOCKS is off.
    pub fn for_cli(key: NetworkKey, settings: &NetSettings) -> NodeConfig {
        NodeConfig {
            network_key: key,
            settings: settings.clone(),
            socks: SocksListen::OFF,
            probe_first_delay: Duration::from_secs(10),
            initial_domains: vec![],
            geoip: true,
            socks_off_by_flag: false,
            proxy_face: None,
            blocked_domains: Vec::new(),
            exit_hosts: HashMap::new(),
            identity_secret: None,
            identity: IdentityState::Temporary(TemporaryReason::Declined),
            initial_peers: vec![],
            network_label: None,
            peers_cache_path: None,
            docs_store_dir: None,
            peer_names_path: None,
            test_net: None,
            open_list: false,
        }
    }

    /// Node settings for tests: the `mem` transport of a per-process shared network, no DHT and no relay.
    pub fn for_loopback_test(key: NetworkKey) -> NodeConfig {
        let mut cfg = NodeConfig::for_cli(key, &loopback_settings());
        cfg.probe_first_delay = Duration::from_secs(0);
        cfg.geoip = false;
        cfg.socks = SocksListen {
            enabled: true,
            port: 0,
            access: crate::config::config_file::ListenAccess::Localhost,
            ifaces: crate::config::config_file::IfaceFilter::Auto,
        };
        cfg.test_net = Some(crate::transport::net::mem::process_net());
        cfg.open_list = true;
        cfg
    }
}

/// `NetSettings` of the loopback preset: no relay and no checks, the `mem` network.
fn loopback_settings() -> NetSettings {
    NetSettings {
        transport_mode: TransportMode::NoRelay,
        probe_interval_sec: 3600,
        checks_enabled: false,
        net_choice: crate::transport::net::NetChoice {
            transports: vec!["mem".to_string()],
            discovery: vec!["invite".to_string(), "cache".to_string()],
            sync: "mesh".to_string(),
        },
        ..NetSettings::default()
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
