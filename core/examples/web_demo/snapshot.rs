//! Assembly of the demo node's `NodeSnapshot`: `demo_snapshot` (the body of the `NodeControl::snapshot` method) and
//! the helpers `demo_self_id`/`demo_route_state`.

use std::collections::BTreeMap;
use std::net::SocketAddr;

use meshtown_core::transport::net::{NodeSecret, PeerId};

use meshtown_core::node::output;
use meshtown_core::node::program_version::current_program_version;
use meshtown_core::proxy::route::{self, Route};
use meshtown_core::web_gui::{
    CheckInfo, ChosenExit, DomainInfo, ExitInfo, LogSummary, MembersInfo, NodeRuntime,
    NodeSnapshot, PathKindView, PeerInfo, RecordInfo, RelayInfo, RouteMode, RouteState, SocksInfo,
    TransportInfo,
};

use crate::members_reply::demo_self_id_full;
use crate::node::{DemoDomain, DemoNode, DemoState};

/// The fixed demo node identity: the same key that `invite()` uses for its address.
fn demo_self_id() -> PeerId {
    NodeSecret::from_bytes(&[0x01u8; 32]).peer_id()
}

/// The demo version of the node's `route_state`: the same `route::choose_route` selection function as in a real
/// node — the members here are a fixed demo list (`st.peers`), always online and always
/// offering an exit, so the page and `web-check` can show all three modes.
fn demo_route_state(st: &DemoState, peers: &[PeerInfo]) -> RouteState {
    let me = demo_self_id();
    let online: std::collections::HashSet<PeerId> = st.peers.iter().map(|p| p.id_full).collect();
    let serving: std::collections::HashMap<PeerId, bool> =
        st.peers.iter().map(|p| (p.id_full, p.exit)).collect();
    let chosen = st.settings.net.route_exits.clone();
    let decision = route::choose_route(&chosen, me, &online, &serving);
    let active = match &decision {
        Route::Exits(list) => Some(route::first_exit(list)),
        Route::Direct | Route::Refuse(_) => None,
    };
    let state = match &decision {
        Route::Direct => RouteMode::Direct,
        Route::Exits(_) => RouteMode::Exit,
        Route::Refuse(_) => RouteMode::Unavailable,
    };
    let exits = chosen
        .iter()
        .map(|&id| {
            let id_full = id.to_string();
            let peer = peers.iter().find(|p| p.id_full == id_full);
            ChosenExit {
                id: output::short_peer_id(&id),
                id_full,
                name: peer.and_then(|p| p.name.clone()),
                online: peer.is_some(),
                exit: peer.and_then(|p| p.exit),
                country: peer.and_then(|p| p.country.clone()),
                active: active == Some(id),
                removed: false,
            }
        })
        .collect();
    RouteState { state, exits }
}

/// The demo node's own row comes first — as in a real node (`self_peer_info` in `node_ctl/snapshot/peers.rs`):
/// `is_self`, always `online`, the id is the same as in the snapshot's `node`, the name comes from the demo settings.
fn demo_self_peer_info(st: &DemoState) -> PeerInfo {
    PeerInfo {
        id: st.boot_id.clone(),
        id_full: demo_self_id_full(&st.boot_id),
        name: Some("finch-unwary".to_string()),
        is_self: true,
        online: true,
        path: None,
        rtt_ms: None,
        exit: Some(st.settings.net.exit_enabled),
        country: None,
        program_version: Some(current_program_version().to_string()),
        checked: 0,
        stale: false,
        last_seen_sec: 0,
        through_peer_bps: 0,
        through_us_bps: 0,
    }
}

fn build_demo_peer_infos(st: &DemoState) -> Vec<PeerInfo> {
    let others = st.peers.iter().map(|p| PeerInfo {
        id: p.id.clone(),
        id_full: p.id_full.to_string(),
        name: p.name.clone(),
        is_self: false,
        online: p.online,
        path: p.path,
        rtt_ms: Some(30),
        exit: Some(p.exit),
        country: (p.name.as_deref() == Some("peer-one")).then(|| "RU".to_string()),
        program_version: (p.name.as_deref() == Some("peer-one")).then(|| "0.0.9".to_string()),
        checked: 0,
        stale: false,
        last_seen_sec: 1,
        through_peer_bps: 0,
        through_us_bps: 0,
    });
    std::iter::once(demo_self_peer_info(st))
        .chain(others)
        .collect()
}

/// The verdict of the last domain check, shown for "self" and for the first demo peer (if any).
fn build_domain_checks(d: &DemoDomain, st: &DemoState) -> BTreeMap<String, Option<CheckInfo>> {
    let mut checks: BTreeMap<String, Option<CheckInfo>> = BTreeMap::new();
    let Some(v) = d.last_verdict else {
        return checks;
    };
    checks.insert(
        st.boot_id.clone(),
        Some(CheckInfo {
            verdict: v,
            ..Default::default()
        }),
    );
    if let Some(p) = st.peers.first() {
        checks.insert(
            p.id.clone(),
            Some(CheckInfo {
                verdict: v,
                ..Default::default()
            }),
        );
    }
    checks
}

fn build_demo_domain_infos(st: &DemoState) -> Vec<DomainInfo> {
    st.domains
        .iter()
        .map(|d| DomainInfo {
            name: d.name.clone(),
            checks: build_domain_checks(d, st),
            checking: d.checking_until.is_some(),
        })
        .collect()
}

fn build_node_runtime(st: &DemoState) -> NodeRuntime {
    NodeRuntime {
        status: st.status,
        id: st.boot_id.clone(),
        id_full: demo_self_id_full(&st.boot_id),
        uptime_sec: st.t as u64,
        rss_kib: 8192,
        rss_peak_kib: 9000,
    }
}

fn build_members_info(st: &DemoState) -> MembersInfo {
    MembersInfo {
        count: st.peers.iter().filter(|p| p.online).count() as u64,
        gossip_neighbors: st.peers.len() as u64,
        voting: 0,
        me_excluded: st.members.me_excluded,
        admitting: 0,
        me_pending: false,
        admission_refused: st.admission_refused,
    }
}

fn build_socks_info(st: &DemoState) -> SocksInfo {
    SocksInfo {
        enabled: st.settings.process.socks_enabled,
        listen: if st.settings.process.socks_enabled {
            Some(format!("127.0.0.1:{}", st.settings.process.socks_port))
        } else {
            None
        },
        pac_url: if st.settings.process.socks_enabled {
            Some(meshtown_core::proxy::pac_url(SocketAddr::from((
                [127, 0, 0, 1],
                st.settings.process.socks_port,
            ))))
        } else {
            None
        },
        access: st.settings.process.socks_access.as_str().to_string(),
        lan: Vec::new(),
        ifaces: None,
    }
}

fn build_exit_info(st: &DemoState) -> ExitInfo {
    ExitInfo {
        enabled: st.settings.net.exit_enabled,
        domains: st.domains.len() as u64,
        clients: Vec::new(),
    }
}

fn build_transport_info(demo: &DemoNode, st: &DemoState) -> TransportInfo {
    TransportInfo {
        self_path: if demo.no_network {
            None
        } else {
            Some(PathKindView::Direct)
        },
        relay: RelayInfo::default(),
        nat: "unknown".to_string(),
        listen_udp: Vec::new(),
        dht: demo.dht_info(),
        record: RecordInfo::default(),
        self_addresses: vec![format!("{}@192.168.1.14:47821", st.boot_id)],
    }
}

fn build_node_snapshot(
    demo: &DemoNode,
    st: &DemoState,
    peers: Vec<PeerInfo>,
    domains: Vec<DomainInfo>,
    route: RouteState,
    log: LogSummary,
) -> NodeSnapshot {
    NodeSnapshot {
        boot_id: st.boot_id.clone(),
        now: st.t,
        node: build_node_runtime(st),
        members: build_members_info(st),
        socks: build_socks_info(st),
        exit: build_exit_info(st),
        transport: build_transport_info(demo, st),
        peers,
        domains,
        route,
        ports: crate::ports::demo_ports(st),
        log,
    }
}

/// The body of `NodeControl::snapshot` for `DemoNode` (the method in `control.rs` calls this function in one
/// line).
pub(crate) fn demo_snapshot(demo: &DemoNode) -> NodeSnapshot {
    let st = demo.state.lock().unwrap();
    let log_summary: LogSummary = demo.log.lock().unwrap().summary(6);
    let peers = build_demo_peer_infos(&st);
    let route = demo_route_state(&st, &peers);
    let domains = build_demo_domain_infos(&st);
    build_node_snapshot(demo, &st, peers, domains, route, log_summary)
}
