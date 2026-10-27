//! Pure assembly of `NodeSnapshot` from already-read data (`Inputs`) — no network and no `.await`.
//! The sources and formulas are documented in the node-web design notes.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::net::SocketAddr;
use std::time::Instant;

use crate::exit::PeerRate;
use crate::transport::net::{PeerAddr, PeerId};

use crate::config::config_file::ListenAccess;
use crate::members::access::report::VoteSummary;
use crate::node::output;
use crate::node::telemetry::TelemetryView;
use crate::site_checks::checks::CheckView;
use crate::site_checks::reachability::{CheckRecord, CheckVerdict};
use crate::transport::net_mainline::record::ENTRY_TTL_SECS;
use crate::web_gui::auth::access::IfaceAddr;
use crate::web_gui::{
    CheckInfo, DhtInfo, DomainInfo, ExitInfo, MembersInfo, NodeRuntime, NodeSnapshot, PathKindView,
    PeerInfo, RecordInfo, RelayInfo, SocksInfo, SocksLan, TransportInfo, VerdictKind,
};

use super::collect::Collected;
use super::peers::build_peers;
#[cfg(test)]
use super::process::link_status;
use super::route_state::route_state;

/// Everything the pure `build` function needs, gathered by `collect` in one pass without repeated
/// `.await`.
pub(super) struct Inputs<'a> {
    pub id: PeerId,
    pub me: PeerId,
    pub self_name: Option<&'a str>,
    pub members: HashSet<PeerId>,
    /// Names of other members — a copy of `Members::names()`; the own name arrives separately
    /// (`self_name`).
    pub peer_names: HashMap<PeerId, String>,
    pub neighbors: u64,
    /// Requests waiting for this node's confirmation and whether this node is out (`access::report::vote_summary`).
    pub votes: VoteSummary,
    /// Nodes with an exclusion record in the verdicts; empty while nothing is published.
    pub excluded: HashSet<PeerId>,
    pub socks: Option<SocketAddr>,
    /// Proxy access mode and this machine's address on the local network (only with `localnet`).
    pub socks_access: (ListenAccess, Vec<IfaceAddr>),
    /// Network filter the proxy was started with (`socks.ifaces`).
    pub socks_ifaces: crate::config::config_file::IfaceFilter,
    pub exit_on: bool,
    /// Raw list of exit clients from the `exit::Clients` registry: unordered, may include non-members.
    pub exit_clients: Vec<PeerId>,
    /// Exit traffic rate per peer, bytes/s (`exit::ExitTraffic::rates_at`).
    pub exit_rates: HashMap<PeerId, PeerRate>,
    pub exits: HashMap<PeerId, bool>,
    /// Member's country — shown for an offline member too.
    pub countries: HashMap<PeerId, String>,
    /// Member's program version; the own one is always `current_program_version()`, inserted by
    /// `collect`; shown for an offline member too.
    pub program_versions: HashMap<PeerId, String>,
    pub chosen_exits: Vec<PeerId>,
    pub domain_names: Vec<String>,
    pub check_views: Vec<CheckView>,
    pub in_progress: HashSet<String>,
    pub telemetry: TelemetryView,
    pub addr: PeerAddr,
    pub now_unix: u64,
    pub now_instant: Instant,
    /// Members from the on-disk address cache (id → `seen_unix`); one of the sources of the liveness flag
    /// and of the `peers[]` rows.
    pub cached_seen: HashMap<PeerId, u64>,
    /// Time of the author's last write in the shared document (unix seconds, per node); a liveness
    /// flag for filtering out departed members, adds no rows to `peers[]`.
    pub doc_seen: HashMap<PeerId, u64>,
}

/// Pure assembly of the snapshot from already-read data — no network and no `.await`.
pub(super) fn build(i: Inputs<'_>) -> Collected {
    let transport = build_transport(&i);
    let peers = build_peers(&i);
    let domains = build_domains(&i, &peers);
    let route = route_state(&i.chosen_exits, &i, &peers);

    let snap = NodeSnapshot {
        node: NodeRuntime {
            id: output::short_peer_id(&i.id),
            id_full: i.id.to_string(),
            ..Default::default()
        },
        members: MembersInfo {
            count: i.members.len() as u64,
            gossip_neighbors: i.neighbors,
            me_excluded: i.votes.me_excluded,
            admitting: i.votes.admitting,
            me_pending: i.votes.me_pending,
            admission_refused: false,
            closed: false,
        },
        socks: SocksInfo {
            enabled: i.socks.is_some(),
            listen: i.socks.map(|a| a.to_string()),
            pac_url: i.socks.map(crate::proxy::pac_url),
            access: i.socks_access.0.as_str().to_string(),
            lan: socks_lan(i.socks, i.socks_access.1.clone()),
            ifaces: i.socks_ifaces.to_state(),
        },
        exit: ExitInfo {
            enabled: i.exit_on,
            domains: i.domain_names.len() as u64,
            clients: exit_clients(&i),
        },
        transport,
        peers,
        domains,
        route,
        ..Default::default()
    };

    Collected {
        snap,
        domains: i.domain_names,
        partial: false,
    }
}

/// `lan` in `socks`: "address:port" and the interface for other devices on the network; empty — the proxy is off
/// or in `localhost` mode (then the address list is empty too).
pub(super) fn socks_lan(socks: Option<SocketAddr>, lan: Vec<IfaceAddr>) -> Vec<SocksLan> {
    let Some(addr) = socks else { return Vec::new() };
    let one = |(iface, ip): IfaceAddr| SocksLan {
        addr: SocketAddr::from((ip, addr.port())).to_string(),
        iface,
    };
    lan.into_iter().map(one).collect()
}

/// Exit clients for `exit.clients`: full ids, network members only, ascending by string, without
/// duplicates; `[]` if the exit is off.
fn exit_clients(i: &Inputs<'_>) -> Vec<String> {
    if !i.exit_on {
        return Vec::new();
    }
    let ids: BTreeSet<String> = i
        .exit_clients
        .iter()
        .filter(|id| i.members.contains(*id))
        .map(|id| id.to_string())
        .collect();
    ids.into_iter().collect()
}

/// Builds `TransportInfo`: the path to itself, relay, DHT, NAT and its own addresses.
fn build_transport(i: &Inputs<'_>) -> TransportInfo {
    let dht_joined = i.telemetry.dht_joined && i.telemetry.last_publish.is_some();
    let secs_ago = |t: Instant| i.now_instant.saturating_duration_since(t).as_secs();
    let last_publish_sec = i
        .telemetry
        .last_publish
        .filter(|_| dht_joined)
        .map(secs_ago);

    TransportInfo {
        self_path: compute_self_path(i),
        relay: RelayInfo {
            connected: false,
            url: None,
        },
        nat: String::new(),
        listen_udp: Vec::new(),
        dht: DhtInfo {
            joined: dht_joined,
            nodes: i.telemetry.dht_nodes,
            last_publish_sec,
        },
        record: RecordInfo {
            published: i.telemetry.record_published,
            ttl_sec: ENTRY_TTL_SECS,
            read_sec: i.telemetry.dht_read.map(|(t, _)| secs_ago(t)),
        },
        self_addresses: super::super::peer_lines(&i.addr),
    }
}

/// Path to itself: `Direct` if at least one member has a `Direct` path (established by `hyper`); otherwise `Relay`
/// if at least one member is reached through a relay; otherwise `None`.
fn compute_self_path(i: &Inputs<'_>) -> Option<PathKindView> {
    let has = |kind: PathKindView| {
        i.members.iter().any(|id: &PeerId| {
            i.telemetry
                .peers
                .get(id)
                .is_some_and(|l| l.path == Some(kind))
        })
    };
    let direct = has(PathKindView::Direct).then_some(PathKindView::Direct);
    direct.or_else(|| has(PathKindView::Relay).then_some(PathKindView::Relay))
}

/// Builds the domain list in `domain_names` order; `checks` is a map of short id → record (`None` if
/// there is no record or its content has not been downloaded yet), with an explicit `None` for every member without a record.
fn build_domains(i: &Inputs<'_>, peers: &[PeerInfo]) -> Vec<DomainInfo> {
    let mut out = Vec::with_capacity(i.domain_names.len());
    for name in &i.domain_names {
        let mut checks: BTreeMap<String, Option<CheckInfo>> = BTreeMap::new();
        for v in &i.check_views {
            if &v.domain != name {
                continue;
            }
            let key = if v.author == i.me {
                output::short_peer_id(&i.id)
            } else {
                output::short_peer_id(&v.author)
            };
            let value = v.record.as_ref().map(|r| check_info(r, i.now_unix));
            checks.insert(key, value);
        }
        for p in peers {
            checks.entry(p.id.clone()).or_insert(None);
        }

        out.push(DomainInfo {
            name: name.clone(),
            checks,
            checking: i.in_progress.contains(name),
        });
    }
    out
}

/// Moves `CheckRecord` into `CheckInfo`: widens `u16`/`u32` to `u64`, converts the time into an age
/// (`now_unix − at`, saturating: a record from the future by the author's clock gives `0`).
fn check_info(r: &CheckRecord, now_unix: u64) -> CheckInfo {
    CheckInfo {
        verdict: verdict_kind(r.verdict),
        reason: r.reason.clone(),
        dns_ms: u64::from(r.dns_ms),
        tcp_ms: u64::from(r.tcp_ms),
        tls_ms: u64::from(r.tls_ms),
        http_ms: u64::from(r.http_ms),
        status: r.status,
        bytes: u64::from(r.bytes),
        stalled: r.stalled,
        age_sec: now_unix.saturating_sub(r.at),
    }
}

/// Mapping `CheckVerdict` → `VerdictKind`. No `_`: a new `CheckVerdict` variant breaks the build.
fn verdict_kind(v: CheckVerdict) -> VerdictKind {
    match v {
        CheckVerdict::Ok => VerdictKind::Ok,
        CheckVerdict::DnsMismatch => VerdictKind::DnsMismatch,
        CheckVerdict::NetworkBlocked => VerdictKind::NetworkBlocked,
        CheckVerdict::SiteGeoblock => VerdictKind::SiteGeoblock,
        CheckVerdict::Antibot => VerdictKind::Antibot,
        CheckVerdict::Down => VerdictKind::Down,
    }
}

#[cfg(test)]
#[path = "build_exit_tests.rs"]
mod exit_tests;

#[cfg(test)]
#[path = "build_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "build_peers_tests.rs"]
mod peers_tests;

#[cfg(test)]
#[path = "build_more_tests.rs"]
mod more_tests;

#[cfg(test)]
#[path = "build_cache_tests.rs"]
mod cache_tests;

#[cfg(test)]
#[path = "build_country_tests.rs"]
mod country_tests;

#[cfg(test)]
#[path = "build_program_version_tests.rs"]
mod program_version_tests;

#[cfg(test)]
#[path = "build_stale_tests.rs"]
mod stale_tests;

#[cfg(test)]
#[path = "pac_url_tests.rs"]
mod pac_url_tests;
