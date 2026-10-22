//! Starting the node with a network key: parsing the key, building `Relaunch` and `NodeConfig`, the page and the node.
//! What is not here: choosing the launch branch and parsing flags — that is in `commands.rs`.

use anyhow::Context;
use meshtown::config_file::{CliOverrides, RunConfig};
use meshtown::host::{Host, PageRole};
use meshtown::network_key::{parse_user_input, NetworkKey};
use meshtown::node::IdentityState;
use meshtown::output::NET_NAME;

use super::file_hint;

/// Parsing the network key: from a flag (`overrides.network_key`) or from the settings file (then the error
/// names the file).
pub(super) fn parse_run_key(
    key_text: &str,
    overrides: &CliOverrides,
    run: &RunConfig,
) -> anyhow::Result<meshtown::network_key::NetworkKeyInput> {
    Ok(if overrides.network_key.is_none() {
        parse_user_input(key_text).with_context(|| {
            format!(
                "Invalid network key from the settings file {}",
                file_hint(&run.file_report)
            )
        })?
    } else {
        parse_user_input(key_text)?
    })
}

/// The "key is ready" log line — only for a key made of words.
pub(super) fn log_key_ready(started: std::time::Instant, from_words: bool, label: Option<&str>) {
    if from_words {
        tracing::debug!(
            "the {NET_NAME} network key is ready in {} µs{}",
            started.elapsed().as_micros(),
            match label {
                Some(l) => format!(", {NET_NAME} network label \"{l}\" (does not affect the key)"),
                None => String::new(),
            }
        );
    }
}

/// Building `Relaunch` and `NodeConfig` for the first run (the label and addresses come from the invite, if there is one).
pub(super) fn launch_parts(
    run: &RunConfig,
    overrides: CliOverrides,
    initial_domains: Vec<String>,
    blocked_domains: Vec<String>,
    network_key: NetworkKey,
    label: Option<String>,
    invited: Option<meshtown::invite::ParsedInvite>,
) -> (meshtown::node_ctl::Relaunch, meshtown::node::NodeConfig) {
    let relaunch = meshtown::node_ctl::Relaunch {
        path: run.file_report.path.clone(),
        cli: overrides,
        secret: run.secret.clone(),
        initial_domains,
        blocked_domains,
        test_net: None,
    };
    let mut cfg = relaunch.node_config(&run.effective_settings, network_key);
    cfg.identity = IdentityState::from_report(&run.file_report);
    let (invite_label, invite_peers) = match invited {
        Some(inv) => (inv.head.label, inv.peers),
        None => (None, Vec::new()),
    };
    cfg.network_label = label.or(invite_label);
    cfg.initial_peers = invite_peers;
    (relaunch, cfg)
}

/// The node page and the node itself: if the page failed to come up, the node runs without it.
async fn serve_node(
    cfg: meshtown::node::NodeConfig,
    run: RunConfig,
    relaunch: meshtown::node_ctl::Relaunch,
) -> anyhow::Result<()> {
    let ctl = meshtown::node_ctl::NodeCtl::new(&run.effective_settings);
    Host::run_with_web_page(run, ctl, PageRole::Optional, |run, ctl| {
        meshtown::node::run_node_with_ctl(cfg, run.effective_settings, ctl, relaunch)
    })
    .await
}

/// The branch where `run.effective_settings.network_key` is set: parses the key (from the flag or the file), starts
/// the node page and the node itself.
pub(super) async fn run_with_key(
    key_text: String,
    overrides: CliOverrides,
    run: RunConfig,
    initial_domains: Vec<String>,
    blocked_domains: Vec<String>,
    invited: Option<meshtown::invite::ParsedInvite>,
) -> anyhow::Result<()> {
    let started = std::time::Instant::now();
    let parsed = parse_run_key(&key_text, &overrides, &run)?;
    let from_words = parsed.from_words;
    let label = parsed.label;
    let network_key = parsed.key;
    log_key_ready(started, from_words, label.as_deref());
    let (relaunch, cfg) = launch_parts(
        &run,
        overrides,
        initial_domains,
        blocked_domains,
        network_key,
        label,
        invited,
    );
    serve_node(cfg, run, relaunch).await
}
