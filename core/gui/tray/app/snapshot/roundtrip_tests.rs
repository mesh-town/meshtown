//! Round-trip tests: the server's real types → JSON → `TraySnapshot`. Renaming a field or changing its type
//! in `meshtown::web` that the tray reads breaks them instead of silently giving a default.

use super::*;
use meshtown::web::{
    ChosenExit, ExitInfo, NodeRuntime, NodeSnapshot, NodeStatus, PeerInfo, RouteMode, RouteState,
    SocksInfo,
};
use serde_json::json;

const ME: &str = "aa00bb11cc22dd33";
const EXIT: &str = "e1e1e1e1e1e1e1e1";

/// The `/api/state` response: the snapshot as JSON plus the `network` key that the page listener appends
/// (`apply_state_extras`; the snapshot types have no such field).
fn over_the_wire(snapshot: &NodeSnapshot, network: Option<serde_json::Value>) -> TraySnapshot {
    let mut value = serde_json::to_value(snapshot).expect("the snapshot serializes");
    if let (Some(network), Some(object)) = (network, value.as_object_mut()) {
        object.insert("network".into(), network);
    }
    TraySnapshot::from_value(&value)
}

fn full_snapshot() -> NodeSnapshot {
    NodeSnapshot {
        node: NodeRuntime {
            status: NodeStatus::Offline,
            id_full: ME.into(),
            uptime_sec: 42,
            ..Default::default()
        },
        socks: SocksInfo { enabled: true, listen: Some("127.0.0.1:1080".into()), pac_url: None, ..Default::default() },
        exit: ExitInfo { enabled: true, domains: 1, clients: vec!["c1".into(), "c2".into()] },
        peers: vec![
            PeerInfo { id: "aa00".into(), id_full: ME.into(), is_self: true, online: true, ..Default::default() },
            PeerInfo {
                id: "e1e1".into(),
                id_full: EXIT.into(),
                name: Some("Berlin".into()),
                online: true,
                exit: Some(true),
                country: Some("DE".into()),
                through_peer_bps: 7,
                through_us_bps: 9,
                ..Default::default()
            },
        ],
        route: RouteState {
            state: RouteMode::Exit,
            exits: vec![ChosenExit {
                id: "e1e1".into(),
                id_full: EXIT.into(),
                name: Some("Berlin".into()),
                online: true,
                country: Some("DE".into()),
                active: true,
                ..Default::default()
            }],
        },
        ..Default::default()
    }
}

#[test]
fn roundtrip_reads_every_tray_field() {
    let snap = over_the_wire(&full_snapshot(), Some(json!(null)));
    let node = snap.node.expect("node");
    assert_eq!(node.status, Some(TrayNodeStatus::Offline));
    assert_eq!(node.uptime_sec, Some(42));
    assert_eq!(node.id_full, ME);
    assert!(snap.network_is_null);
    assert!(snap.socks.enabled);
    assert_eq!(snap.socks.listen.as_deref(), Some("127.0.0.1:1080"));
    assert!(snap.exit.enabled);
    assert_eq!(snap.exit.clients, vec!["c1".to_string(), "c2".to_string()]);
    assert_eq!(snap.route.state, RouteMode::Exit);
    let chosen = &snap.route.exits[0];
    assert_eq!(chosen.id.as_deref(), Some("e1e1"));
    assert_eq!(chosen.id_full.as_deref(), Some(EXIT));
    assert_eq!(chosen.name.as_deref(), Some("Berlin"));
    assert_eq!(chosen.country.as_deref(), Some("DE"));
    assert!(chosen.online && chosen.active);
    let peers = snap.peers.expect("peers");
    assert!(peers[0].is_self && peers[0].online);
    assert_eq!(peers[0].id_full.as_deref(), Some(ME));
    assert_eq!(peers[1].id.as_deref(), Some("e1e1"));
    assert_eq!(peers[1].name.as_deref(), Some("Berlin"));
    assert_eq!(peers[1].country.as_deref(), Some("DE"));
    assert!(peers[1].online && peers[1].exit && !peers[1].is_self);
    assert_eq!((peers[1].through_peer_bps, peers[1].through_us_bps), (Some(7), Some(9)));
}

#[test]
fn roundtrip_none_options_stay_none() {
    let mut snapshot = full_snapshot();
    snapshot.route.exits[0].name = None;
    snapshot.route.exits[0].country = None;
    snapshot.peers[1].name = None;
    snapshot.peers[1].country = None;
    snapshot.peers[1].exit = None;
    snapshot.socks.listen = None;
    let snap = over_the_wire(&snapshot, None);
    assert!(!snap.network_is_null);
    assert_eq!(snap.socks.listen, None);
    assert_eq!((snap.route.exits[0].name.clone(), snap.route.exits[0].country.clone()), (None, None));
    let peer = &snap.peers.expect("peers")[1];
    assert_eq!((peer.name.clone(), peer.country.clone()), (None, None));
    assert!(!peer.exit);
}

#[test]
fn roundtrip_status_words_match_server() {
    for status in [NodeStatus::Starting, NodeStatus::Online, NodeStatus::Offline, NodeStatus::Stopping] {
        let mut snapshot = NodeSnapshot::default();
        snapshot.node.status = status;
        let word = over_the_wire(&snapshot, None).node.and_then(|n| n.status);
        let expected = match status {
            NodeStatus::Starting => TrayNodeStatus::Starting,
            NodeStatus::Online => TrayNodeStatus::Online,
            NodeStatus::Offline => TrayNodeStatus::Offline,
            NodeStatus::Stopping => TrayNodeStatus::Stopping,
        };
        assert_eq!(word, Some(expected));
    }
}

#[test]
fn roundtrip_route_modes_match_server() {
    for mode in [RouteMode::Direct, RouteMode::Exit, RouteMode::Unavailable] {
        let mut snapshot = NodeSnapshot::default();
        snapshot.route.state = mode;
        assert_eq!(over_the_wire(&snapshot, None).route.state, mode);
    }
}
