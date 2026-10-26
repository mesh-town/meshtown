//! Tests for `snapshot.rs`: a "golden" set of JSON keys of the `/api/state` response.
//!
//! The page and scripts read these keys by name, so any change to the node's internals must
//! keep their set unchanged. The file is not edited after the golden set was recorded: the constants `DEFAULT_KEYS` and
//! `FILLED_KEYS` were captured by a run, not derived from the structs. The `apply_state_extras` keys
//! (`web/routes/state.rs`) are not part of the golden set.
//! The `socks.pac_url` key was added by hand — a new field, the old keys were
//! not touched.
//! The `peers[].through_peer_bps` and `peers[].through_us_bps` keys were added by hand — new fields, the old keys were not touched.
//! The `members.voting` and `members.me_excluded` keys were added by hand — new fields,
//! the old keys were not touched.

use super::*;
use crate::ports_sharing::{PortReach, Proto};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Paths of dictionaries whose keys are data (identifiers), not field names: their key in the path is `{}`.
const DICT_PATHS: &[&str] = &["domains[].checks"];

/// Key paths of an empty snapshot: arrays are empty, `Option` is `null`.
const DEFAULT_KEYS: &[&str] = &[
    "boot_id",
    "domains",
    "exit",
    "exit.clients",
    "exit.domains",
    "exit.enabled",
    "log",
    "log.first_seq",
    "log.last_seq",
    "log.ring_lines",
    "log.tags",
    "log.tail",
    "members",
    "members.admission_refused",
    "members.admitting",
    "members.count",
    "members.gossip_neighbors",
    "members.me_excluded",
    "members.me_pending",
    "members.voting",
    "node",
    "node.id",
    "node.id_full",
    "node.rss_kib",
    "node.rss_peak_kib",
    "node.status",
    "node.uptime_sec",
    "now",
    "peers",
    "ports",
    "ports.network",
    "ports.published",
    "route",
    "route.exits",
    "route.state",
    "socks",
    "socks.access",
    "socks.enabled",
    "socks.ifaces",
    "socks.lan",
    "socks.listen",
    "socks.pac_url",
    "transport",
    "transport.dht",
    "transport.dht.joined",
    "transport.dht.last_publish_sec",
    "transport.dht.nodes",
    "transport.listen_udp",
    "transport.nat",
    "transport.record",
    "transport.record.published",
    "transport.record.read_sec",
    "transport.record.ttl_sec",
    "transport.relay",
    "transport.relay.connected",
    "transport.relay.url",
    "transport.self_addresses",
    "transport.self_path",
];

/// Key paths of a filled snapshot: every `Option` = `Some`, every `Vec` has one element.
const FILLED_KEYS: &[&str] = &[
    "boot_id",
    "domains",
    "domains[].checking",
    "domains[].checks",
    "domains[].checks.{}",
    "domains[].checks.{}.age_sec",
    "domains[].checks.{}.bytes",
    "domains[].checks.{}.dns_ms",
    "domains[].checks.{}.http_ms",
    "domains[].checks.{}.reason",
    "domains[].checks.{}.stalled",
    "domains[].checks.{}.status",
    "domains[].checks.{}.tcp_ms",
    "domains[].checks.{}.tls_ms",
    "domains[].checks.{}.verdict",
    "domains[].name",
    "exit",
    "exit.clients",
    "exit.domains",
    "exit.enabled",
    "log",
    "log.first_seq",
    "log.last_seq",
    "log.ring_lines",
    "log.tags",
    "log.tail",
    "log.tail[].seq",
    "log.tail[].t",
    "log.tail[].tag",
    "log.tail[].text",
    "members",
    "members.admission_refused",
    "members.admitting",
    "members.count",
    "members.gossip_neighbors",
    "members.me_excluded",
    "members.me_pending",
    "members.voting",
    "node",
    "node.id",
    "node.id_full",
    "node.rss_kib",
    "node.rss_peak_kib",
    "node.status",
    "node.uptime_sec",
    "now",
    "peers",
    "peers[].checked",
    "peers[].country",
    "peers[].exit",
    "peers[].id",
    "peers[].id_full",
    "peers[].last_seen_sec",
    "peers[].name",
    "peers[].online",
    "peers[].path",
    "peers[].rtt_ms",
    "peers[].self",
    "peers[].stale",
    "peers[].through_peer_bps",
    "peers[].through_us_bps",
    "peers[].version",
    "ports",
    "ports.network",
    "ports.network[].name",
    "ports.network[].node",
    "ports.network[].node_name",
    "ports.network[].online",
    "ports.network[].opened",
    "ports.network[].opened.error",
    "ports.network[].opened.last_error",
    "ports.network[].opened.local",
    "ports.network[].opened.state",
    "ports.network[].port",
    "ports.network[].proto",
    "ports.network[].published",
    "ports.network[].web_url",
    "ports.published",
    "ports.published[].local",
    "ports.published[].name",
    "ports.published[].port",
    "ports.published[].proto",
    "route",
    "route.exits",
    "route.exits[].active",
    "route.exits[].country",
    "route.exits[].exit",
    "route.exits[].id",
    "route.exits[].id_full",
    "route.exits[].name",
    "route.exits[].online",
    "route.state",
    "socks",
    "socks.access",
    "socks.enabled",
    "socks.ifaces",
    "socks.lan",
    "socks.listen",
    "socks.pac_url",
    "transport",
    "transport.dht",
    "transport.dht.joined",
    "transport.dht.last_publish_sec",
    "transport.dht.nodes",
    "transport.listen_udp",
    "transport.nat",
    "transport.record",
    "transport.record.published",
    "transport.record.read_sec",
    "transport.record.ttl_sec",
    "transport.relay",
    "transport.relay.connected",
    "transport.relay.url",
    "transport.self_addresses",
    "transport.self_path",
];

#[test]
fn api_state_keys_of_default_snapshot() {
    // given
    let snapshot = NodeSnapshot::default();

    // when
    let keys = key_paths(&to_json(&snapshot));

    // then
    assert_same_keys(&keys, DEFAULT_KEYS);
}

#[test]
fn api_state_keys_of_filled_snapshot() {
    // given
    let snapshot = filled_snapshot();

    // when
    let keys = key_paths(&to_json(&snapshot));

    // then
    assert_same_keys(&keys, FILLED_KEYS);
}

#[test]
fn path_kinds_serialize_lowercase() {
    // given
    let mut snapshot = NodeSnapshot::default();
    snapshot.transport.self_path = Some(PathKindView::Direct);
    snapshot.peers = vec![
        PeerInfo {
            path: Some(PathKindView::Relay),
            ..Default::default()
        },
        PeerInfo {
            path: None,
            ..Default::default()
        },
    ];

    // when
    let json = to_json(&snapshot);

    // then
    assert_eq!(json["transport"]["self_path"], "direct");
    assert_eq!(json["peers"][0]["path"], "relay");
    assert!(
        json["peers"][1]["path"].is_null(),
        "path is not determined — `null`, not a missing key"
    );
}

fn to_json(snapshot: &NodeSnapshot) -> Value {
    serde_json::to_value(snapshot).expect("snapshot serializes")
}

/// Sorted set of key paths: objects — via a dot, an array — the `[]` segment (walking the
/// first element), a dictionary with data keys (`DICT_PATHS`) — the `{}` segment.
fn key_paths(json: &Value) -> Vec<String> {
    let mut paths = BTreeSet::new();
    collect_paths(json, "", &mut paths);
    paths.into_iter().collect()
}

fn collect_paths(value: &Value, prefix: &str, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) if DICT_PATHS.contains(&prefix) => {
            let Some(first) = map.values().next() else {
                return;
            };
            let path = format!("{prefix}.{{}}");
            out.insert(path.clone());
            collect_paths(first, &path, out);
        }
        Value::Object(map) => {
            for (key, child) in map {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                out.insert(path.clone());
                collect_paths(child, &path, out);
            }
        }
        Value::Array(items) => {
            if let Some(first) = items.first() {
                collect_paths(first, &format!("{prefix}[]"), out);
            }
        }
        _ => {}
    }
}

fn assert_same_keys(actual: &[String], golden: &[&str]) {
    let extra: Vec<&String> = actual
        .iter()
        .filter(|k| !golden.contains(&k.as_str()))
        .collect();
    let missing: Vec<&&str> = golden
        .iter()
        .filter(|k| !actual.iter().any(|a| a == **k))
        .collect();
    assert!(
        extra.is_empty() && missing.is_empty(),
        "the /api/state key set diverged from the golden set\nextra keys: {extra:#?}\nmissing keys: {missing:#?}"
    );
    assert_eq!(
        actual.len(),
        golden.len(),
        "the golden set has repeated paths"
    );
}

fn text(s: &str) -> String {
    s.to_string()
}

/// Every `Option` = `Some`, every `Vec` is one element, the checks dictionary is one entry.
fn filled_snapshot() -> NodeSnapshot {
    NodeSnapshot {
        socks: SocksInfo {
            enabled: true,
            listen: Some(text("127.0.0.1:1080")),
            pac_url: Some(text("http://127.0.0.1:1080/proxy.pac")),
            ifaces: Some(vec![text("br-lan")]),
            ..Default::default()
        },
        exit: ExitInfo {
            enabled: true,
            domains: 1,
            clients: vec![text("client")],
        },
        transport: filled_transport(),
        peers: vec![filled_peer()],
        domains: vec![filled_domain()],
        route: filled_route(),
        ports: filled_ports(),
        log: LogSummary {
            tags: vec![text("web")],
            tail: vec![LogLine::default()],
            ..Default::default()
        },
        ..Default::default()
    }
}

fn filled_transport() -> TransportInfo {
    TransportInfo {
        self_path: Some(PathKindView::Direct),
        relay: RelayInfo {
            connected: true,
            url: Some(text("https://relay.example")),
        },
        listen_udp: vec![text("0.0.0.0:1")],
        dht: DhtInfo {
            joined: true,
            nodes: 1,
            last_publish_sec: Some(1),
        },
        record: RecordInfo {
            read_sec: Some(1),
            ..Default::default()
        },
        self_addresses: vec![text("192.0.2.1:1")],
        ..Default::default()
    }
}

fn filled_peer() -> PeerInfo {
    PeerInfo {
        name: Some(text("peer")),
        path: Some(PathKindView::Relay),
        rtt_ms: Some(1),
        exit: Some(true),
        country: Some(text("RU")),
        program_version: Some(text("0.1.0")),
        ..Default::default()
    }
}

fn filled_domain() -> DomainInfo {
    DomainInfo {
        checks: BTreeMap::from([(text("exit"), Some(CheckInfo::default()))]),
        ..Default::default()
    }
}

fn filled_route() -> RouteState {
    let exit = ChosenExit {
        name: Some(text("exit")),
        exit: Some(true),
        country: Some(text("RU")),
        ..Default::default()
    };
    RouteState {
        state: RouteMode::Exit,
        exits: vec![exit],
    }
}

fn filled_ports() -> PortsInfo {
    let opened = OpenedInfo {
        local: 1,
        state: text("bind_failed"),
        error: Some(text("busy")),
        last_error: Some(PortReach::Unsupported),
    };
    let network = NetPort {
        node: text("node"),
        node_name: Some(text("peer")),
        online: true,
        port: 1,
        proto: Proto::Udp,
        name: text("web"),
        published: true,
        opened: Some(opened),
        web_url: Some(text("url")),
    };
    PortsInfo {
        published: vec![OwnPort {
            port: 1,
            proto: Proto::Tcp,
            name: text("web"),
            local: text("ok"),
        }],
        network: vec![network],
    }
}
