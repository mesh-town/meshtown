//! Node settings for assembly (`NodeConfig`): the preset for the CLI and the preset for loopback tests.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use crate::config::config_file::NetSettings;
use crate::transport::net::{NodeSecret, PeerAddr, PeerId};

use crate::members::network_key::{NetworkKey, NetworkKeyInput};

use super::identity::{IdentityState, TemporaryReason};

/// Node settings for assembly.
#[derive(Clone, Debug)]
pub struct NodeConfig {
    pub network_key: NetworkKey,
    /// The network part of the settings file; the node reads it only from here.
    pub settings: NetSettings,
    pub probe_first_delay: Duration,
    /// Domains the node adds to its list at startup.
    pub initial_domains: Vec<String>,
    /// Whether to ask external sources for our own country and announce it to peers.
    pub geoip: bool,
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
    /// The settings file of the slot this node runs; the invited peers file beside it is removed once the
    /// membership boot has applied it. `None` — nothing to remove (tests, `--no-config`).
    pub invited_peers_file: Option<PathBuf>,
    /// The network label from the invite or the settings file.
    pub network_label: Option<String>,
    /// The root of the network: its first admin, from the key.
    pub net_root: PeerId,
    /// The secret an invite redemption answers with.
    pub reply_secret: NetworkKey,
    /// File of the peer address cache; `None` — the cache is off (tests, `--no-config`).
    pub peers_cache_path: Option<PathBuf>,
    /// Directory of the document and blob store (the domain list); `None` — in memory (tests,
    /// `--no-config`).
    pub docs_store_dir: Option<PathBuf>,
    /// File of the last known member names; `None` — neither read nor written (tests,
    /// `--no-config`).
    pub peer_names_path: Option<PathBuf>,
    /// File of the saved DHT table; `None` — not read and not written (tests, `--no-config`).
    pub dht_table_path: Option<PathBuf>,
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
    pub fn for_cli(key: NetworkKeyInput, settings: &NetSettings) -> NodeConfig {
        let reply = NetworkKey::from_bytes(key.reply_secret());
        let mut cfg = assemble(key.key.clone(), key.root, reply, settings);
        cfg.network_label = key.label;
        cfg
    }

    /// Node settings for tests: the `mem` transport of a per-process shared network, no DHT and no relay.
    pub fn for_loopback_test(key: NetworkKey) -> NodeConfig {
        let reply = key.clone();
        let mut cfg = assemble(key, loopback_root(), reply, &loopback_settings());
        cfg.probe_first_delay = Duration::from_secs(0);
        cfg.geoip = false;
        cfg.test_net = Some(crate::transport::net::mem::process_net());
        cfg.open_list = true;
        cfg
    }
}

/// The node that is the root of every loopback test network.
#[doc(hidden)]
pub fn loopback_root() -> PeerId {
    NodeSecret::from_bytes(&[31; 32]).peer_id()
}

/// The common body of the presets: live defaults around the key, its root and the reply secret.
fn assemble(
    key: NetworkKey,
    root: PeerId,
    reply: NetworkKey,
    settings: &NetSettings,
) -> NodeConfig {
    NodeConfig {
        reply_secret: reply,
        network_key: key,
        settings: settings.clone(),
        probe_first_delay: Duration::from_secs(10),
        initial_domains: vec![],
        geoip: true,
        proxy_face: None,
        blocked_domains: Vec::new(),
        exit_hosts: HashMap::new(),
        identity_secret: None,
        identity: IdentityState::Temporary(TemporaryReason::Declined),
        initial_peers: vec![],
        invited_peers_file: None,
        network_label: None,
        net_root: root,
        peers_cache_path: None,
        docs_store_dir: None,
        peer_names_path: None,
        dht_table_path: None,
        test_net: None,
        open_list: false,
    }
}

/// `NetSettings` of the loopback preset: no checks, the `mem` network.
fn loopback_settings() -> NetSettings {
    NetSettings {
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
