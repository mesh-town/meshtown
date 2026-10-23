//! Reads the running node and telemetry in one pass (`Inputs`), without the pure assembly logic —
//! that lives in `build.rs`.

use crate::transport::net::PeerId;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::config::config_file::{IfaceFilter, ListenAccess};
use crate::members::access::report::{members_reply, vote_summary, VoteSummary};
use crate::members::stale_peers;
use crate::node::node_state;
use crate::node::program_version;
use crate::node::RunningNode;
use crate::proxy::ProxyFace;
use crate::ports_sharing::PortView;
use crate::site_checks::geoip;
use crate::web_gui::auth::access::{lan_addrs, IfaceAddr};
use crate::web_gui::auth::iface_nets::admitted_lan;
use crate::web_gui::auth::iface_os::current_view;
use crate::web_gui::NodeStatus;
use crate::web_gui::{MembersInfo, NodeSnapshot, PortsInfo};

use super::build::{build, Inputs};
use super::doc_cache::{DocCache, DocParts};
use super::ports::build_ports;

/// Timeout for each document read (`domains.list()`, `checks.check_views()`, `ports_doc.read_all()`, etc.).
const READ_TIMEOUT: Duration = Duration::from_millis(800);

/// Addresses of this machine on the networks open to the proxy (empty with `localhost`).
fn socks_lan_addrs(access: ListenAccess, ifaces: &IfaceFilter) -> Vec<IfaceAddr> {
    admitted_lan(access, ifaces, &current_view(), lan_addrs())
}

/// Result of one collection: the snapshot, the list of known domain names (needed only by the action gates of
/// `node_ctl` — `check_domain`/`check_all`/`add_domain`/`remove_domain`) and the flag of a partial
/// collection (the domains could not be fetched — the caller keeps the previous list).
pub(crate) struct Collected {
    pub snap: NodeSnapshot,
    pub domains: Vec<String>,
    pub partial: bool,
}

/// Reads a document under `READ_TIMEOUT`: an error or timeout gives `empty` and `false`.
async fn read_or<T, E>(
    fut: impl std::future::Future<Output = Result<T, E>>,
    empty: T,
) -> (T, bool) {
    match tokio::time::timeout(READ_TIMEOUT, fut).await {
        Ok(Ok(v)) => (v, true),
        _ => (empty, false),
    }
}

/// Program versions of members from the shared document and the read-success flag (without the own version).
async fn program_versions_of(node: &RunningNode) -> (HashMap<PeerId, String>, bool) {
    read_or(
        program_version::read_program_versions(node.domains.shared_replica().as_ref()),
        HashMap::new(),
    )
    .await
}

/// Time of each author's last write in the document and the read-success flag.
async fn doc_seen_of(node: &RunningNode, now_unix: u64) -> (HashMap<PeerId, u64>, bool) {
    let r = node.domains.shared_replica();
    read_or(
        stale_peers::read_doc_seen(r.as_ref(), now_unix),
        HashMap::new(),
    )
    .await
}

/// Seven document reads under a timeout (`domains.list()`, `checks.check_views()`,
/// `node_state::read_exit_flags`, `geoip::read_countries`, program versions, `ports_doc.read_all()`,
/// `read_doc_seen`) and the "all succeeded" flag. Called only by `DocCache::read_doc_parts_cached`.
pub(super) async fn read_doc_parts(node: &RunningNode, now_unix: u64) -> (DocParts, bool) {
    let (list, list_ok) = read_or(node.domains.list(), Vec::new()).await;
    let (views, views_ok) = read_or(node.checks.check_views(), Vec::new()).await;
    let r = node.domains.shared_replica();
    let (exits, exits_ok) = read_or(node_state::read_exit_flags(r.as_ref()), HashMap::new()).await;
    let (countries, countries_ok) =
        read_or(geoip::read_countries(r.as_ref()), HashMap::new()).await;
    let (program_versions, versions_ok) = program_versions_of(node).await;
    let (port_views, ports_ok) = read_or(node.ports_doc.read_all(), Vec::new()).await;
    let (doc_seen, doc_seen_ok) = doc_seen_of(node, now_unix).await;
    let read_complete =
        list_ok && views_ok && exits_ok && countries_ok && ports_ok && versions_ok && doc_seen_ok;
    let parts = DocParts {
        list,
        views,
        exits,
        countries,
        program_versions,
        port_views,
        doc_seen,
    };
    (parts, read_complete)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn country_texts(countries: HashMap<PeerId, geoip::Country>) -> HashMap<PeerId, String> {
    countries
        .into_iter()
        .map(|(id, c)| (id, c.as_str().to_string()))
        .collect()
}

/// Collects the node snapshot: document parts go through the `cache` (`DocCache`), the rest is
/// synchronous reads of live state and the own program version on every tick, then the pure
/// `build`. An error or timeout of any document read leaves the matching field empty and sets
/// `partial: true` (such a read does not enter the cache); nothing is logged (when the node stops
/// this is expected).
pub(crate) async fn collect(
    node: &RunningNode,
    self_name: Option<&str>,
    mode: &str,
    cache: &mut DocCache,
) -> Collected {
    let now_unix = unix_now();
    let (parts, partial) = cache.read_doc_parts_cached(node, now_unix).await;
    let DocParts {
        list,
        views,
        exits,
        countries,
        mut program_versions,
        port_views,
        doc_seen,
    } = parts;
    program_versions.insert(
        node.id,
        program_version::current_program_version().to_string(),
    );

    let face = node.proxy_face.borrow().clone();
    let inputs = Inputs {
        id: node.id,
        me: node.domains.me(),
        self_name,
        mode,
        members: node.members.ids(),
        peer_names: node.members.names(),
        neighbors: node.gossip_neighbor_count.load(Ordering::Relaxed) as u64,
        votes: votes_of(node),
        socks: face.addr,
        socks_access: socks_access_of(&face),
        socks_ifaces: face.ifaces,
        exit_on: node.exit_on.load(Ordering::Relaxed),
        exit_clients: node.exit_clients.active_peers_at(Instant::now()),
        exit_rates: node.exit_traffic.rates_at(Instant::now()),
        exits,
        countries: country_texts(countries),
        program_versions,
        chosen_exits: node.route.chosen_exits(),
        domain_names: list,
        check_views: views,
        in_progress: node.checks.domains_in_progress(),
        telemetry: node.telemetry.view(),
        addr: crate::invite::own_addr(&node.stack, node.id),
        now_unix,
        now_instant: Instant::now(),
        cached_seen: (*node.cached_peers).clone(),
        doc_seen,
    };

    let mut collected = build(inputs);
    collected.snap.ports = ports_of(node, &port_views);
    collected.partial = partial;
    collected
}

/// The published ports with their live state on this node.
fn ports_of(node: &RunningNode, port_views: &[PortView]) -> PortsInfo {
    build_ports(
        port_views,
        node.id,
        &node.members.names(),
        &node.members.ids(),
        node.ports.published_with_state(),
        node.opener.bind_states(),
    )
}

/// The SOCKS access mode together with the LAN addresses it is reachable on.
fn socks_access_of(face: &ProxyFace) -> (ListenAccess, Vec<IfaceAddr>) {
    (face.access, socks_lan_addrs(face.access, &face.ifaces))
}

/// The published verdicts as a summary for the page; nothing published yet — an empty one. Never recomputes.
fn votes_of(node: &RunningNode) -> VoteSummary {
    let Some(v) = node.access.view.verdicts() else {
        return VoteSummary::default();
    };
    vote_summary(&members_reply(
        &v,
        &node.members,
        &node.access.view.refusals(),
        None,
    ))
}

/// Rewrites the members block of a snapshot from the live node: the count, the gossip neighbours and the
/// votes are read at every call, so a page opened after a pause does not show a stale block.
pub(in crate::node::node_ctl) fn overlay_members(snap: &mut NodeSnapshot, node: &RunningNode) {
    let votes = votes_of(node);
    snap.members = MembersInfo {
        count: node.members.count() as u64,
        gossip_neighbors: node.gossip_neighbor_count.load(Ordering::Relaxed) as u64,
        voting: votes.voting,
        me_excluded: votes.me_excluded,
        admitting: votes.admitting,
        me_pending: votes.me_pending,
    };
}

/// Node status from the last collection: online if there are network members or the
/// relay is connected, otherwise offline. `Starting`/`Stopping` on top is decided by `NodeCtl::snapshot()` — they are not here.
pub(crate) fn link_status(c: &Collected) -> NodeStatus {
    if c.snap.members.count > 0 || c.snap.transport.relay.connected {
        NodeStatus::Online
    } else {
        NodeStatus::Offline
    }
}

#[cfg(test)]
#[path = "collect_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "collect_country_tests.rs"]
mod country_tests;

#[cfg(test)]
#[path = "collect_program_version_tests.rs"]
mod program_version_tests;

#[cfg(test)]
#[path = "collect_votes_tests.rs"]
mod votes_tests;
