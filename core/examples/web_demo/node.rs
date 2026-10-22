//! State of the demo node: `DemoNode`, its nested structures, and the constructor `DemoNode::new`.

use std::sync::Mutex;

use tokio::sync::Notify;

use meshtown_core::transport::net::{NodeSecret, PeerId};

use meshtown_core::config::config_file::Settings;
use meshtown_core::node::output;
use meshtown_core::web_gui::{DhtInfo, LogRing, NodeStatus, PathKindView, VerdictKind};

use crate::members::DemoMembers;
use crate::rng::gen_hex8;

pub(crate) struct DemoPeer {
    pub(crate) id: String,
    pub(crate) id_full: PeerId,
    pub(crate) name: Option<String>,
    pub(crate) path: Option<PathKindView>,
    pub(crate) online: bool,
}

pub(crate) struct DemoDomain {
    pub(crate) name: String,
    pub(crate) checking_until: Option<f64>,
    pub(crate) last_verdict: Option<VerdictKind>,
}

pub(crate) struct DemoState {
    pub(crate) t: f64,
    pub(crate) fast: f64,
    pub(crate) boot_id: String,
    pub(crate) status: NodeStatus,
    pub(crate) gen: u64,
    pub(crate) peers: Vec<DemoPeer>,
    pub(crate) domains: Vec<DemoDomain>,
    pub(crate) members: DemoMembers,
    pub(crate) settings: Settings,
    pub(crate) starting_until: Option<f64>,
    pub(crate) next_line_at: f64,
    pub(crate) next_status_at: f64,
}

/// Demo-only parameters of the web-check scenarios that a real node does not have
/// (how many peers are offline and the DHT status).
pub(crate) struct DemoNetworkOptions {
    pub(crate) offline_peers: u64,
    pub(crate) dht_joined: bool,
    pub(crate) dht_nodes: u64,
    /// `--members-second-vote`: a second member agrees to every removal proposal at once.
    pub(crate) members_second_vote: bool,
    /// `--me-excluded`: the node starts already removed from the network.
    pub(crate) me_excluded: bool,
    /// `--members-pending`: a node that asked to join waits for confirmations.
    pub(crate) members_pending: bool,
    /// `--me-pending`: the node itself waits for confirmations.
    pub(crate) me_pending: bool,
}

pub(crate) struct DemoNode {
    pub(crate) state: Mutex<DemoState>,
    pub(crate) log: Mutex<LogRing>,
    pub(crate) notify: Notify,
    pub(crate) no_network: bool,
    dht_joined: bool,
    dht_nodes: u64,
}

/// Startup log: a fixed feed of ten "ready" lines, so the page immediately sees
/// varied tags before the first tick of the background task.
fn build_startup_log(ring_capacity: usize) -> LogRing {
    let mut log = LogRing::new(ring_capacity.max(1));
    let startup_tags = [
        "node", "dht", "member", "gossip", "socks", "check", "node", "dht", "member", "node",
    ];
    for (i, tag) in startup_tags.iter().enumerate() {
        log.push(0.1 * (i as f64 + 1.0), tag, format!("start: {tag} ready"));
    }
    log
}

/// Two fixed demo peers; `offline_peers` determines how many of them are shown offline.
fn build_demo_peers(offline_peers: u64, _gen: &mut u64) -> Vec<DemoPeer> {
    let peer_count = 2u64;
    let offline_from = peer_count.saturating_sub(offline_peers.min(peer_count));
    // Detail 5: the member code in the "Members" table and in
    // `#route-state` when an exit is chosen must be one and the same code of one
    // and the same id — before, `id` came from an independent `gen_hex8`, now from
    // the same `id_full` as `ChosenExit.id` (`output::short_peer_id`, see `snapshot.rs`).
    let peer_one_id_full = NodeSecret::from_bytes(&[0x02u8; 32]).peer_id();
    let peer_two_id_full = NodeSecret::from_bytes(&[0x03u8; 32]).peer_id();
    vec![
        DemoPeer {
            id: output::short_peer_id(&peer_one_id_full),
            id_full: peer_one_id_full,
            name: Some("peer-one".to_string()),
            path: Some(PathKindView::Direct),
            online: 0 < offline_from,
        },
        DemoPeer {
            id: output::short_peer_id(&peer_two_id_full),
            id_full: peer_two_id_full,
            name: Some("peer-two".to_string()),
            path: Some(PathKindView::Relay),
            online: 1 < offline_from,
        },
    ]
}

/// Three fixed demo domains with no checks started.
fn build_demo_domains() -> Vec<DemoDomain> {
    ["example.com", "example.org", "x.com"]
        .into_iter()
        .map(|d| DemoDomain {
            name: d.to_string(),
            checking_until: None,
            last_verdict: None,
        })
        .collect()
}

/// Empty with `no_network`; otherwise the fixed demo peers and demo domains.
fn build_demo_peers_and_domains(
    no_network: bool,
    offline_peers: u64,
    gen: &mut u64,
) -> (Vec<DemoPeer>, Vec<DemoDomain>) {
    if no_network {
        return (Vec::new(), Vec::new());
    }
    (build_demo_peers(offline_peers, gen), build_demo_domains())
}

impl DemoNode {
    pub(crate) fn new(
        seed: u64,
        fast: f64,
        ring_capacity: usize,
        no_network: bool,
        net_opts: DemoNetworkOptions,
        settings: Settings,
    ) -> Self {
        let mut gen = if seed == 0 { 1 } else { seed };
        let boot_id = gen_hex8(&mut gen);
        let log = build_startup_log(ring_capacity);
        let (peers, domains) =
            build_demo_peers_and_domains(no_network, net_opts.offline_peers, &mut gen);
        let members = DemoMembers::new(&peers, &net_opts);

        DemoNode {
            state: Mutex::new(DemoState {
                t: 0.0,
                fast,
                boot_id,
                status: NodeStatus::Online,
                gen,
                peers,
                domains,
                members,
                settings,
                starting_until: None,
                next_line_at: 0.5,
                next_status_at: 60.0,
            }),
            log: Mutex::new(log),
            notify: Notify::new(),
            no_network,
            dht_joined: net_opts.dht_joined,
            dht_nodes: net_opts.dht_nodes,
        }
    }

    pub(crate) fn push_log(&self, t: f64, tag: &str, text: String) {
        self.log.lock().unwrap().push(t, tag, text);
    }

    /// `--dht-joined`/`--dht-nodes` (demo-only) instead of a hard-coded `DhtInfo::default()`.
    pub(crate) fn dht_info(&self) -> DhtInfo {
        DhtInfo {
            joined: self.dht_joined,
            nodes: self.dht_nodes,
            last_publish_sec: None,
        }
    }
}
