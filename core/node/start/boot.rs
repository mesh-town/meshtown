//! The first start phase: the SOCKS5 port, the secret, transports, the node name and identity; the banner and
//! listen addresses to the log. `Boot` is the intermediate state carried onward through the
//! start phases by the other files of `node/start/`.

use std::sync::Arc;

use crate::transport::net::{NodeSecret, PeerId, Transport};
use anyhow::Result;
use data_encoding::HEXLOWER;
use tokio::net::TcpListener;

use crate::config::config_file::NetSettings;
use crate::members::network_key::{NetworkKey, SubKeys};
use crate::members::words::trim_and_validate_node_name;
use crate::node::node_name::effective_node_name;
use crate::node::output::{say, short_peer_id};
use crate::node::telemetry::Telemetry;
use crate::proxy;

use crate::node::config::NodeConfig;
use crate::node::identity::IdentityState;

use super::transports::bind_transports;

/// The intermediate start state: transports and everything known right after they come up,
/// plus handles of background tasks that accumulate across the start phases and go to `RunningNode.background_abort_handles`.
pub(super) struct Boot {
    pub(super) background_abort_handles: Vec<tokio::task::AbortHandle>,
    pub(super) socks_listener: Option<TcpListener>,
    pub(super) subkeys: SubKeys,
    pub(super) secret: NodeSecret,
    /// The stack transports in `net.transports` order.
    pub(super) transports: Vec<Arc<dyn Transport>>,
    /// The typed `hyper` handle, if it is in `net.transports`.
    #[cfg(feature = "net-hyper")]
    pub(super) hyper: Option<Arc<crate::transport::net_hyper::HyperTransport>>,
    pub(super) id: PeerId,
    pub(super) name: String,
    pub(super) identity: IdentityState,
    pub(super) network_key: NetworkKey,
    pub(super) network_label: Option<String>,
    pub(super) telemetry: Arc<Telemetry>,
}

/// The SOCKS5 port, the secret, transports, the node name and identity; the banner and listen addresses
/// to the log.
pub(super) async fn boot_endpoint(cfg: &NodeConfig) -> Result<Boot> {
    let socks_listener = bind_socks_listener(cfg).await?;
    let subkeys = cfg.network_key.subkeys();
    let secret = resolve_secret(cfg)?;
    let bound = bind_transports(cfg, &secret).await?;
    let id = secret.peer_id();
    let (name, name_check) = pick_node_name(cfg, id);
    let identity = cfg.identity;
    log_identity_banner(cfg, &subkeys, id, &name, name_check, identity);
    let telemetry = Telemetry::new();
    Ok(Boot {
        background_abort_handles: Vec::new(),
        socks_listener,
        subkeys,
        secret,
        transports: bound.list,
        #[cfg(feature = "net-hyper")]
        hyper: bound.hyper,
        id,
        name,
        identity,
        network_key: cfg.network_key.clone(),
        network_label: cfg.network_label.clone(),
        telemetry,
    })
}

/// The SOCKS5 port is taken before the network: if it is busy the node doesn't start, so as not to
/// end up with a running node without a proxy.
async fn bind_socks_listener(cfg: &NodeConfig) -> Result<Option<TcpListener>> {
    Ok(match cfg.socks.enabled {
        true => Some(proxy::bind_proxy_listener(cfg.socks.port, cfg.socks.access).await?),
        false => None,
    })
}

/// The node secret: from the settings file, otherwise a new one (tests, `--no-config`).
fn resolve_secret(cfg: &NodeConfig) -> Result<NodeSecret> {
    Ok(match &cfg.identity_secret {
        Some(s) => s.clone(),
        None => crate::config::config_file::new_secret()?,
    })
}

/// The node name: from the settings file if it passes the check, otherwise the default from the key. The second
/// element is the check result (for the line about an unsuitable name in the banner).
fn pick_node_name(cfg: &NodeConfig, id: PeerId) -> (String, Option<Result<&str, &'static str>>) {
    let name_check = cfg
        .settings
        .node_name
        .as_deref()
        .map(trim_and_validate_node_name);
    let name = effective_node_name(cfg.settings.node_name.as_deref(), id);
    (name, name_check)
}

/// The start lines: version, network, node, identity; parsing the name from the settings file, the relay mode.
fn log_identity_banner(
    cfg: &NodeConfig,
    subkeys: &SubKeys,
    id: PeerId,
    name: &str,
    name_check: Option<Result<&str, &'static str>>,
    identity: IdentityState,
) {
    let net_id_hex = subkeys.network_id_hex();
    let lines = identity_lines(&cfg.settings, &net_id_hex, id, name, name_check, identity);
    lines.into_iter().for_each(|line| say("node", line));
}

/// The banner lines in order: the banner, "name is not suitable" (if any), the identity text (if any), the mode.
fn identity_lines(
    settings: &NetSettings,
    net_id_hex: &str,
    id: PeerId,
    name: &str,
    name_check: Option<Result<&str, &'static str>>,
    identity: IdentityState,
) -> Vec<String> {
    [
        Some(banner_line(net_id_hex, id, name, identity)),
        bad_name_line(name_check, name),
        identity.warning_text(),
        Some(mode_line(settings)),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// The first line: version, network, node, identity.
fn banner_line(net_id_hex: &str, id: PeerId, name: &str, identity: IdentityState) -> String {
    format!(
        "meshtown {}, network {} {}, node {} \"{}\" ({}), identity {}",
        env!("CARGO_PKG_VERSION"),
        crate::node::output::NET_NAME,
        net_id_hex,
        short_peer_id(&id),
        name,
        HEXLOWER.encode(id.as_bytes()),
        identity.kind_word(),
    )
}

/// The line about an unsuitable name from the settings file; present only on `Some(Err(..))`.
fn bad_name_line(name_check: Option<Result<&str, &'static str>>, name: &str) -> Option<String> {
    let why = name_check?.err()?;
    Some(format!(
        "name from the settings file (node.name) is not valid: {why}; \
using name \"{name}\" for this run"
    ))
}

/// The transport mode line.
fn mode_line(settings: &NetSettings) -> String {
    let mode = mode_word(settings);
    format!("mode: {mode}")
}

fn mode_word(settings: &NetSettings) -> &'static str {
    if !settings.uses_relay() {
        return "no relay";
    }
    if settings.is_relay_only() {
        return "relay only";
    }
    "direct + relay"
}

#[cfg(test)]
#[path = "boot_tests.rs"]
mod tests;
