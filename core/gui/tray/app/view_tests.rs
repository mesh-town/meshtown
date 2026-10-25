//! Tests of `view`: the node state and the facts from the JSON snapshot — `start_enabled`/`stop_enabled`, `tray_color`,
//! `facts_from_json`.

use super::*;
use serde_json::Value;

/// A shim with the old name: tests supply JSON, the code reads the selection.
fn facts_from_json(value: &serde_json::Value) -> Option<ColorFacts> {
    color_facts(&crate::gui::tray::app::snapshot::TraySnapshot::from_value(
        value,
    ))
}

const API_EXAMPLE: &str = r#"{
  "boot_id": "b3f1c2a9",
  "now": 1861.4,
  "config_rev": 7,
  "node": {
    "status": "online",
    "version": "0.5.0",
    "id": "254b24c7d9",
    "id_full": "254b24c7d9e8...",
    "uptime_sec": 67320,
    "rss_kib": 14648,
    "rss_peak_kib": 16486,
    "web_addr": "127.0.0.1:7777",
    "identity": "persistent",
    "identity_reason": null
  },
  "network": {
    "label": "our-gaming-room",
    "masked": "our-gaming-room--••••••••••••••••••••••••••••••••••",
    "id_hex": "9c1e..."
  },
  "members": { "count": 3, "gossip_neighbors": 2 },
  "socks": { "enabled": true, "listen": "127.0.0.1:1080" },
  "exit": { "enabled": true, "ports": [80, 443], "domains": 7 },
  "transport": {
    "mode": "direct_and_relay",
    "self_path": "direct",
    "relay": { "connected": true, "url": "relay.example" },
    "nat": "address-restricted cone",
    "listen_udp": ["192.168.1.14:47821", "[::]:47821"],
    "dht": { "joined": true, "nodes": 8, "last_publish_sec": 137 },
    "record": { "published": true, "ttl_sec": 900 },
    "self_addresses": [
      "254b24c7d9@192.168.1.14:47821",
      "254b24c7d9@77.88.21.3:47821",
      "254b24c7d9@relay:euw1-1"
    ]
  },
  "peers": [
    {
      "id": "254b24c7d9", "id_full": "254b24c7d9e8...", "name": "finch-unwary-4c7d9",
      "self": true, "online": true,
      "path": null, "rtt_ms": null, "exit": true,
      "checked": 7, "stale": false, "last_seen_sec": 0
    },
    {
      "id": "9f4a0c71e8", "id_full": "9f4a0c71e8fullid", "name": "red-pond-9f4a0",
      "self": false, "online": true,
      "path": "relay", "rtt_ms": 186, "exit": true,
      "checked": 5, "stale": false, "last_seen_sec": 12
    },
    {
      "id": "c03b55d2af", "id_full": "c03b55d2affullid", "name": "willow-candid-c03b5",
      "self": false, "online": false,
      "path": null, "rtt_ms": null, "exit": true,
      "checked": 7, "stale": true, "last_seen_sec": 254
    }
  ],
  "route": {
    "state": "exit",
    "exits": [
      {
        "id": "9f4a0c71e8", "id_full": "9f4a0c71e8fullid", "name": "red-pond-9f4a0",
        "online": true, "exit": true, "active": true
      }
    ]
  },
  "domains": [
    {
      "name": "example.org",
      "checks": {
        "254b24c7d9": {
          "verdict": "network_blocked", "reason": "TLS connection reset",
          "dns_ms": 21, "tcp_ms": 34, "tls_ms": 0, "http_ms": 0,
          "status": 0, "bytes": 0, "stalled": false, "age_sec": 142
        },
        "1bd2ec32a5": {
          "verdict": "ok", "reason": "HTTP 200",
          "dns_ms": 18, "tcp_ms": 29, "tls_ms": 41, "http_ms": 248,
          "status": 200, "bytes": 5765, "stalled": false, "age_sec": 96
        },
        "9f4a0c71e8": { "verdict": "ok", "reason": "HTTP 200", "dns_ms": 22, "tcp_ms": 44, "tls_ms": 60, "http_ms": 310, "status": 200, "bytes": 5420, "stalled": false, "age_sec": 300 },
        "c03b55d2af": null
      },
      "checking": false
    }
  ],
  "log": {
    "tags": ["auth","check","checks","console","dht","docs","domains","exit","get","gossip","member","net","node","path","probe","relay","route","socks","status"],
    "ring_lines": 2000,
    "first_seq": 1,
    "last_seq": 4127,
    "tail": [
      { "seq": 4122, "t": 141.3, "tag": "socks", "text": "example.com:443 via 9f4a0c71e8: success, accepted 18204 bytes, sent 921 bytes, 571 ms" },
      { "seq": 4123, "t": 180.0, "tag": "status", "text": "members 3, gossip peers 2, domains 7, checks 14, memory 14712 KiB" }
    ]
  }
}"#;

#[test]
fn initial_is_grey_starting() {
    let view = View::initial();
    assert_eq!(view.color, TrayColor::Grey);
    assert_eq!(view.page, PageState::Starting);
    assert_eq!(view.node, NodeState::Unknown);
}

const ALL_NODE: [NodeState; 5] = [
    NodeState::Unknown,
    NodeState::Absent,
    NodeState::Starting,
    NodeState::Running,
    NodeState::Stopping,
];

#[test]
fn start_enabled_only_absent() {
    for node in ALL_NODE {
        assert_eq!(start_enabled(node), node == NodeState::Absent, "{node:?}");
    }
}

#[test]
fn stop_enabled_starting_running() {
    for node in ALL_NODE {
        let expected = matches!(node, NodeState::Starting | NodeState::Running);
        assert_eq!(stop_enabled(node), expected, "{node:?}");
    }
}

#[test]
fn node_text_matches_state() {
    assert_eq!(
        node_state_phrase(NodeState::Unknown),
        "Mesh Town not visible"
    );
    assert_eq!(
        node_state_phrase(NodeState::Absent),
        "Mesh Town is not running"
    );
    assert_eq!(
        node_state_phrase(NodeState::Starting),
        "Mesh Town is starting"
    );
    assert_eq!(
        node_state_phrase(NodeState::Running),
        "Mesh Town is running"
    );
    assert_eq!(
        node_state_phrase(NodeState::Stopping),
        "Mesh Town is stopping"
    );
}

#[test]
fn tray_color_none_is_grey() {
    assert_eq!(tray_color(None), TrayColor::Grey);
}

#[test]
fn tray_color_offline_forces_grey() {
    let facts = ColorFacts {
        online: false,
        exit_available: true,
        via_peer: true,
    };
    assert_eq!(tray_color(Some(facts)), TrayColor::Grey);
}

#[test]
fn tray_color_online_no_exit_is_red() {
    let facts = ColorFacts {
        online: true,
        exit_available: false,
        via_peer: false,
    };
    assert_eq!(tray_color(Some(facts)), TrayColor::Red);
}

#[test]
fn tray_color_online_via_peer_is_green() {
    let facts = ColorFacts {
        online: true,
        exit_available: true,
        via_peer: true,
    };
    assert_eq!(tray_color(Some(facts)), TrayColor::Green);
}

#[test]
fn facts_from_json_api_example_is_online_with_route_exit() {
    let value: Value = serde_json::from_str(API_EXAMPLE).expect("the fixture is valid JSON");
    let facts = facts_from_json(&value).expect("the node is online, network is not null");
    assert_eq!(
        facts,
        ColorFacts {
            online: true,
            exit_available: true,
            via_peer: true,
        }
    );
}

#[test]
fn facts_from_json_route_state_unavailable_is_red() {
    // given: the node is online, but none of the selected exits is online
    let value = serde_json::json!({
        "node": { "status": "online" },
        "route": { "state": "unavailable", "exits": [] }
    });

    // when
    let facts = facts_from_json(&value).expect("node.status is a string");

    // then
    assert_eq!(tray_color(Some(facts)), TrayColor::Red);
}

#[test]
fn facts_from_json_route_state_direct_is_green_ring() {
    let value = serde_json::json!({
        "node": { "status": "online" },
        "route": { "state": "direct", "exits": [] }
    });
    let facts = facts_from_json(&value).expect("node.status is a string");
    assert_eq!(tray_color(Some(facts)), TrayColor::GreenOutline);
}

#[test]
fn facts_from_json_route_state_exit_is_green() {
    let value = serde_json::json!({
        "node": { "status": "online" },
        "route": {
            "state": "exit",
            "exits": [
                { "id": "a", "id_full": "afull", "name": null, "online": true, "exit": true, "active": true }
            ]
        }
    });
    let facts = facts_from_json(&value).expect("node.status is a string");
    assert_eq!(tray_color(Some(facts)), TrayColor::Green);
}

#[test]
fn facts_from_json_route_missing_is_treated_as_direct_and_green_ring() {
    // given: a snapshot of an old node with no "route" key at all
    let value = serde_json::json!({ "node": { "status": "online" } });

    let facts = facts_from_json(&value).expect("node.status is a string");

    assert_eq!(tray_color(Some(facts)), TrayColor::GreenOutline);
}

#[test]
fn facts_from_json_network_null_forces_offline() {
    let value = serde_json::json!({
        "node": { "status": "online" },
        "network": null,
        "peers": [
            { "id": "a", "self": false, "online": true, "exit": true }
        ]
    });
    let facts = facts_from_json(&value).expect("node.status is a string");
    assert!(!facts.online);
}

#[test]
fn facts_from_json_network_absent_keeps_status() {
    let value = serde_json::json!({
        "node": { "status": "online" },
        "peers": []
    });
    let facts = facts_from_json(&value).expect("node.status is a string");
    assert!(facts.online);
}

#[test]
fn facts_from_json_missing_node_is_none() {
    let value = serde_json::json!({ "network": null });
    assert_eq!(facts_from_json(&value), None);
}
