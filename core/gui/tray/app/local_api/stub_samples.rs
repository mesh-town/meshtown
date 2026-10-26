//! Builders of the JSON the node sends to the tray: `/api/state` and `/api/overview`.

use serde_json::{json, Value};

/// A peer in the `peers` array of `/api/state`.
#[derive(Clone)]
pub(crate) struct PeerSpec<'a> {
    pub(crate) id: &'a str,
    pub(crate) name: &'a str,
    pub(crate) online: bool,
    pub(crate) exit: bool,
    pub(crate) country: Option<&'a str>,
}

impl<'a> PeerSpec<'a> {
    pub(crate) fn new(name: &'a str) -> Self {
        PeerSpec {
            id: name,
            name,
            online: true,
            exit: false,
            country: None,
        }
    }
}

/// The parts of `/api/state` the tray reads. `chosen` is the id of the selected exit (one of `peers`).
pub(crate) struct StateSpec<'a> {
    pub(crate) status: &'a str,
    pub(crate) network: bool,
    pub(crate) peers: Vec<PeerSpec<'a>>,
    pub(crate) chosen: Option<&'a str>,
    pub(crate) serves_exit: bool,
    pub(crate) socks: Option<&'a str>,
}

impl Default for StateSpec<'_> {
    fn default() -> Self {
        StateSpec {
            status: "online",
            network: true,
            peers: vec![],
            chosen: None,
            serves_exit: false,
            socks: None,
        }
    }
}

pub(crate) fn state_json(s: &StateSpec) -> Value {
    let peer = |p: &PeerSpec| json!({"id": p.id, "id_full": p.id, "name": p.name, "country": p.country, "self": false, "online": p.online, "exit": p.exit});
    let picked = |p: &&PeerSpec| Some(p.id) == s.chosen;
    let exits: Vec<Value> = s.peers.iter().filter(picked).map(|p| {
        json!({"id": p.id, "id_full": p.id, "name": p.name, "country": p.country, "online": p.online, "exit": true, "active": p.online})
    }).collect();
    let route = if exits.is_empty() { "direct" } else { "exit" };
    json!({
        "node": {"status": s.status, "id_full": "me00", "uptime_sec": 999},
        "network": if s.network { json!({"name": "x"}) } else { Value::Null },
        "peers": s.peers.iter().map(peer).collect::<Vec<_>>(),
        "route": {"state": route, "exits": exits},
        "exit": {"enabled": s.serves_exit, "clients": []},
        "socks": {"enabled": s.socks.is_some(), "listen": s.socks},
        "members": {"admitting": 0},
    })
}

/// One network of `/api/overview`: the summary entry and the slot with its `/api/state`.
pub(crate) struct NetSpec<'a> {
    pub(crate) id: &'a str,
    pub(crate) label: Option<&'a str>,
    pub(crate) id_hex: Option<&'a str>,
    pub(crate) status: &'a str,
    pub(crate) proxy: bool,
    pub(crate) joining: bool,
    pub(crate) peers_online: u32,
    pub(crate) peers_total: u32,
    pub(crate) admitting: u32,
    pub(crate) route_state: &'a str,
    pub(crate) exits: Vec<&'a str>,
    pub(crate) state: Option<Value>,
}

impl Default for NetSpec<'_> {
    fn default() -> Self {
        NetSpec {
            id: "main",
            label: Some("friends"),
            id_hex: Some("aa"),
            status: "online",
            proxy: false,
            joining: false,
            peers_online: 2,
            peers_total: 3,
            admitting: 0,
            route_state: "direct",
            exits: vec![],
            state: Some(state_json(&StateSpec::default())),
        }
    }
}

impl NetSpec<'_> {
    /// A network the node holds no key for: it has no label and its state has no network.
    pub(crate) fn keyless(id: &'static str) -> NetSpec<'static> {
        let state = StateSpec {
            network: false,
            ..Default::default()
        };
        NetSpec {
            id,
            label: None,
            id_hex: None,
            state: Some(state_json(&state)),
            ..Default::default()
        }
    }

    /// A slot that could not be read: no label, status `broken`, no state.
    pub(crate) fn broken(id: &'static str) -> NetSpec<'static> {
        NetSpec {
            id,
            label: None,
            id_hex: None,
            status: "broken",
            peers_online: 0,
            peers_total: 0,
            route_state: "unavailable",
            state: None,
            ..Default::default()
        }
    }
}

pub(crate) fn overview_json(specs: &[NetSpec]) -> Value {
    let net = |n: &NetSpec| {
        json!({
            "id": n.id, "label": n.label, "id_hex": n.id_hex, "status": n.status, "exit_enabled": false,
            "proxy": n.proxy, "route_state": n.route_state, "exits": n.exits,
            "joining": n.joining, "members_admitting": n.admitting,
            "me_excluded": false, "me_pending": false,
            "peers_online": n.peers_online, "peers_total": n.peers_total,
        })
    };
    let slot = |n: &NetSpec| json!({"slot": n.id, "state": n.state, "members": null});
    json!({
        "api": 2,
        "nets": specs.iter().map(net).collect::<Vec<_>>(),
        "slots": specs.iter().map(slot).collect::<Vec<_>>(),
        "restart_slots": [],
    })
}

/// The main network with the exit peer `peer-one` (country `RU`) and the peer `peer-three` that is offline.
pub(crate) fn friends() -> NetSpec<'static> {
    let one = PeerSpec {
        exit: true,
        country: Some("RU"),
        ..PeerSpec::new("peer-one")
    };
    let three = PeerSpec {
        online: false,
        ..PeerSpec::new("peer-three")
    };
    let state = StateSpec {
        peers: vec![one, PeerSpec::new("peer-two"), three],
        ..Default::default()
    };
    NetSpec {
        proxy: true,
        state: Some(state_json(&state)),
        ..Default::default()
    }
}

/// An added network `work` with the exit peer `peer-b` already chosen.
pub(crate) fn work() -> NetSpec<'static> {
    let b = PeerSpec {
        id: "peerb",
        exit: true,
        ..PeerSpec::new("peer-b")
    };
    let state = StateSpec {
        peers: vec![b],
        chosen: Some("peerb"),
        ..Default::default()
    };
    NetSpec {
        id: "ab12cd",
        label: Some("work"),
        exits: vec!["peerb"],
        route_state: "exit",
        state: Some(state_json(&state)),
        ..Default::default()
    }
}

const PEER_NAMES: [&str; 10] = [
    "peer-0", "peer-1", "peer-2", "peer-3", "peer-4", "peer-5", "peer-6", "peer-7", "peer-8",
    "peer-9",
];

/// The main network whose state lists `n` (at most ten) participants, none of them an exit.
pub(crate) fn friends_with_peers(n: usize) -> NetSpec<'static> {
    let peers = PEER_NAMES[..n]
        .iter()
        .map(|name| PeerSpec::new(name))
        .collect();
    let state = StateSpec {
        peers,
        ..Default::default()
    };
    NetSpec {
        state: Some(state_json(&state)),
        ..friends()
    }
}

/// A network `label` in slot `id` with no participants at all.
pub(crate) fn empty_net(id: &'static str, label: &'static str) -> NetSpec<'static> {
    NetSpec {
        id,
        label: Some(label),
        ..Default::default()
    }
}
