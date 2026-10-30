//! The member and resolver contexts built for a new node.

use portable_atomic::AtomicU64;
use std::collections::HashSet;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::time::Duration;

use crate::members::access::view::AccessView;
use crate::members::membership::{MemberContext, MemberEvent, Members};
use crate::members::network_key::SubKeys;
use crate::node::config::NodeConfig;
use crate::site_checks::checks;
use crate::site_checks::reachability::CheckConfig;
use crate::transport::net::PeerId;
use crate::transport::transfer_probe::ProbeSettings;
use anyhow::{Context, Result};
use tokio::sync::{mpsc, watch};

/// The member context and the volume-probe settings for a new node.
pub(super) fn build_member_context(
    access: AccessView,
    id: PeerId,
    subkeys: &SubKeys,
    name: String,
    probe_first_delay: Duration,
    start_probe_interval_secs: u64,
    (exits, full_cycle): (watch::Receiver<Vec<PeerId>>, bool),
) -> (
    Members,
    mpsc::UnboundedReceiver<MemberEvent>,
    Arc<AtomicUsize>,
    Arc<AtomicU64>,
    MemberContext,
) {
    let (members, event_rx, event_tx) = new_members(access);
    let probe = new_probe_settings(
        probe_first_delay,
        start_probe_interval_secs,
        (exits, full_cycle),
    );
    let ok_probe_count = probe.ok_probe_count.clone();
    let probe_interval_secs = probe.interval_secs.clone();
    let ctx = MemberContext {
        own_id: id,
        handshake_key: subkeys.handshake_key,
        members: members.clone(),
        events: event_tx,
        probe,
        own_name_tx: tokio::sync::watch::channel(name).0,
    };
    (members, event_rx, ok_probe_count, probe_interval_secs, ctx)
}

/// Volume-probe settings with fresh counters.
fn new_probe_settings(
    first_delay: Duration,
    start_interval_secs: u64,
    (exits, full_cycle): (watch::Receiver<Vec<PeerId>>, bool),
) -> ProbeSettings {
    // The live probe interval handle: the same `Arc` goes into `ProbeSettings` and, through the
    // caller, into `RunningNode.probe_interval_secs` at the end of `start`.
    ProbeSettings {
        first_delay,
        interval_secs: Arc::new(AtomicU64::new(start_interval_secs.max(1))),
        ok_probe_count: Arc::new(AtomicUsize::new(0)),
        exits,
        full_cycle,
    }
}

/// `Members` with the given access view and the channel of their events.
fn new_members(
    access: AccessView,
) -> (
    Members,
    mpsc::UnboundedReceiver<MemberEvent>,
    mpsc::UnboundedSender<MemberEvent>,
) {
    let members = Members::with_access(access);
    let (event_tx, event_rx) = mpsc::unbounded_channel::<MemberEvent>();
    (members, event_rx, event_tx)
}

/// Blocked domains, check settings and the node's shared resolver (one
/// resolver for checks and for the direct proxy path; the exit uses a copy
/// of it with its own cache, bound to the exit interface).
pub(super) fn build_resolver_context(
    cfg: &NodeConfig,
) -> Result<(
    Arc<HashSet<String>>,
    CheckConfig,
    Arc<crate::site_checks::doh::Resolver>,
)> {
    let (blocked_domains, check_cfg) = check_settings(cfg);
    let resolver = Arc::new(
        crate::site_checks::doh::Resolver::new(
            cfg.settings.doh_server,
            check_cfg.limits,
            check_cfg.user_agent.clone(),
        )
        .context("node resolver")?,
    );
    Ok((blocked_domains, check_cfg, resolver))
}

/// The block list and the check settings that carry it.
fn check_settings(cfg: &NodeConfig) -> (Arc<HashSet<String>>, CheckConfig) {
    let blocked_domains: Arc<HashSet<String>> =
        Arc::new(cfg.blocked_domains.iter().cloned().collect());
    let check_cfg = CheckConfig {
        user_agent: checks::DEFAULT_USER_AGENT.to_string(),
        provider_dns: cfg.settings.provider_dns,
        geo_markers: cfg.settings.geo_markers.iter().cloned().collect(),
        blocked_domains: blocked_domains.clone(),
        ..CheckConfig::default()
    };
    (blocked_domains, check_cfg)
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
