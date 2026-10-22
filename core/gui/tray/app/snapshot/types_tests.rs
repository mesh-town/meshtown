//! Tests of the snapshot selection: malformed input shapes do not break parsing and give the same field default.

use super::*;
use crate::web_gui::RouteMode;
use serde_json::json;

fn snap(value: serde_json::Value) -> TraySnapshot {
    TraySnapshot::from_value(&value)
}

#[test]
fn non_object_body_is_empty_snapshot() {
    for body in [json!([]), json!(5), json!(null)] {
        assert_eq!(snap(body), TraySnapshot::default());
    }
}

#[test]
fn extra_keys_are_ignored() {
    let s = snap(json!({"node": {"status": "online", "rss_kib": 5}, "domains": [1], "x": {}}));
    assert_eq!(s.node.expect("node").status, Some(TrayNodeStatus::Online));
}

#[test]
fn unknown_status_is_unknown_word() {
    let s = snap(json!({"node": {"status": "weird"}}));
    assert_eq!(s.node.expect("node").status, Some(TrayNodeStatus::Unknown));
}

#[test]
fn status_not_string_is_none() {
    for status in [json!(5), json!(null), json!(["online"])] {
        let s = snap(json!({"node": {"status": status}}));
        assert_eq!(s.node.expect("node").status, None);
    }
}

#[test]
fn node_not_object_is_none() {
    for node in [json!(5), json!("x"), json!(null), json!(["online"])] {
        assert_eq!(snap(json!({"node": node})).node, None);
    }
}

#[test]
fn network_four_cases() {
    let cases = [
        (json!({}), false),
        (json!({"network": null}), true),
        (json!({"network": "x"}), false),
        (json!({"network": {}}), false),
    ];
    for (body, expected) in cases {
        assert_eq!(snap(body).network_is_null, expected);
    }
}

#[test]
fn peers_not_array_is_none_empty_is_some() {
    assert_eq!(snap(json!({"peers": "x"})).peers, None);
    assert_eq!(snap(json!({})).peers, None);
    assert_eq!(snap(json!({"peers": []})).peers, Some(vec![]));
}

#[test]
fn peer_not_object_is_default_peer() {
    let s = snap(json!({"peers": [5, [true, true, true]]}));
    assert_eq!(
        s.peers,
        Some(vec![TrayPeer::default(), TrayPeer::default()])
    );
}

#[test]
fn peer_exit_null_is_false() {
    let s = snap(json!({"peers": [{"exit": null}]}));
    assert!(!s.peers.expect("peers")[0].exit);
}

#[test]
fn peer_online_wrong_type_is_false() {
    let s = snap(json!({"peers": [{"online": "yes"}, {"online": 1}]}));
    assert!(s.peers.expect("peers").iter().all(|p| !p.online));
}

#[test]
fn peer_self_absent_is_foreign() {
    let s = snap(json!({"peers": [{"id_full": "a"}, {"self": true}]}));
    let peers = s.peers.expect("peers");
    assert!(!peers[0].is_self);
    assert!(peers[1].is_self);
}

#[test]
fn exit_clients_bad_shapes() {
    assert!(snap(json!({"exit": {"clients": "a"}}))
        .exit
        .clients
        .is_empty());
    let s = snap(json!({"exit": {"clients": ["a", 5, "b"]}}));
    assert_eq!(s.exit.clients, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn route_bad_state_is_direct_and_active_defaults_false() {
    for state in [json!("weird"), json!(5), json!(null)] {
        let s = snap(json!({"route": {"state": state, "exits": [{"id_full": "a"}]}}));
        assert_eq!(s.route.state, RouteMode::Direct);
        assert!(!s.route.exits[0].active);
    }
    assert_eq!(snap(json!({})).route.state, RouteMode::Direct);
}

#[test]
fn socks_listen_null_is_none() {
    let s = snap(json!({"socks": {"enabled": true, "listen": null}}));
    assert!(s.socks.enabled);
    assert_eq!(s.socks.listen, None);
}

#[test]
fn wrong_type_numbers_and_ids() {
    for uptime in [json!(-1), json!(1.5)] {
        let s = snap(json!({"node": {"uptime_sec": uptime, "id_full": 5}}));
        let node = s.node.expect("node");
        assert_eq!(node.uptime_sec, None);
        assert_eq!(node.id_full, "");
    }
}

#[test]
fn rates_missing_are_none() {
    let s = snap(json!({"peers": [{"id": "a"}]}));
    let peer = &s.peers.expect("peers")[0];
    assert_eq!((peer.through_peer_bps, peer.through_us_bps), (None, None));
}

#[test]
fn rates_of_wrong_type_are_none() {
    let s = snap(json!({"peers": [{"through_peer_bps": "abc", "through_us_bps": "abc"}]}));
    let peer = &s.peers.expect("peers")[0];
    assert_eq!((peer.through_peer_bps, peer.through_us_bps), (None, None));
}

#[test]
fn members_read_voting_and_exclusion() {
    let s = snap(json!({"members": {"voting": 3, "me_excluded": true, "count": 5}}));
    assert_eq!(
        s.members,
        TrayMembers {
            voting: 3,
            me_excluded: true,
            ..TrayMembers::default()
        }
    );
}

#[test]
fn members_missing_or_foreign_types_give_defaults() {
    assert_eq!(snap(json!({})).members, TrayMembers::default());
    for members in [json!(5), json!(null), json!([1]), json!("x")] {
        assert_eq!(
            snap(json!({"members": members})).members,
            TrayMembers::default()
        );
    }
    let s = snap(json!({"members": {"voting": "2", "me_excluded": "yes"}}));
    assert_eq!(s.members, TrayMembers::default());
    let s = snap(json!({"members": {"voting": -1}}));
    assert_eq!(s.members.voting, 0);
}
