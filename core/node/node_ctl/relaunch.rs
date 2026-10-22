//! Settings for rebuilding `NodeConfig` on a network switch: the node identity (`secret`) and
//! the task domains survive a network switch, everything else is taken from the new `Settings`.

use crate::transport::net::NodeSecret;
use std::path::PathBuf;

use anyhow::Context;

use crate::config::config_file::{self, CliOverrides, LoadedConfigFile, Settings};
use crate::config::docs_store;
use crate::members::network_key::{self, NetworkKey};
use crate::members::peer_names;
use crate::node::output::{say, NET_NAME};
use crate::node::NodeConfig;
use crate::transport::peers_cache;

pub struct Relaunch {
    pub path: Option<PathBuf>,
    pub cli: CliOverrides,
    pub secret: NodeSecret,
    pub initial_domains: Vec<String>,
    pub blocked_domains: Vec<String>,
    /// The tests' `mem` network: with `Some`, the node built by `node_config` runs on `mem` (transport, lookup, sync
    /// as in `for_loopback_test`), and `invite_wait` takes it for a temporary transport. In production — `None`.
    #[doc(hidden)]
    pub test_net: Option<std::sync::Arc<crate::transport::net::mem::MemNet>>,
}

impl Relaunch {
    pub fn node_config(&self, s: &Settings, key: NetworkKey) -> NodeConfig {
        let mut cfg = s.node_config(key, self.secret.clone());
        cfg.initial_domains = self.initial_domains.clone();
        cfg.blocked_domains = self.blocked_domains.clone();
        cfg.socks_off_by_flag = self.cli.socks_enabled == Some(false);
        cfg.peers_cache_path = self
            .path
            .as_deref()
            .and_then(peers_cache::peers_cache_path_beside);
        cfg.peer_names_path = self
            .path
            .as_deref()
            .and_then(peer_names::peer_names_path_beside);
        cfg.docs_store_dir = self
            .path
            .as_deref()
            .and_then(docs_store::docs_store_path_beside);
        if let Some(net) = &self.test_net {
            cfg.test_net = Some(net.clone());
            cfg.open_list = true;
            cfg.settings.net_choice = NodeConfig::for_loopback_test(cfg.network_key.clone())
                .settings
                .net_choice;
        }
        cfg
    }

    /// Parses the new network key, takes the base settings (file or `prev`), lays the CLI over them
    /// (except the key — its only source is the `key_text` argument) and builds `NodeConfig`. The file
    /// is never written — `reload` only reads.
    pub(crate) fn reload(
        &self,
        key_text: &str,
        prev: &Settings,
    ) -> anyhow::Result<(Settings, NodeConfig)> {
        let parsed = network_key::parse_user_input(key_text).with_context(|| {
            format!("the key of the new {NET_NAME} network could not be parsed")
        })?;
        let mut s = match &self.path {
            None => prev.clone(),
            Some(path) => match config_file::load_config_file(path) {
                LoadedConfigFile::Parsed(p) => p.settings,
                other => {
                    let reason = match other {
                        LoadedConfigFile::Absent => "no file".to_string(),
                        LoadedConfigFile::Unreadable(r) | LoadedConfigFile::Invalid(r) => r,
                        LoadedConfigFile::Parsed(_) => unreachable!(),
                    };
                    say(
                        "node",
                        format!("settings file not read ({reason}); keeping the previous settings"),
                    );
                    prev.clone()
                }
            },
        };
        let mut cli = self.cli.clone();
        cli.network_key = None;
        cli.apply(&mut s);
        s.net.network_key = Some(key_text.to_string());
        let mut cfg = self.node_config(&s, parsed.key);
        cfg.network_label = parsed.label;
        Ok((s, cfg))
    }
}

#[cfg(test)]
#[path = "relaunch_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "relaunch_process_tests.rs"]
mod process_tests;
