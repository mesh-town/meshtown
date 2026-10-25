//! Tests for parsing the `/api/state` snapshot into a `Ring`: the exit, members, the "exits through us" flag.

use super::*;
use serde_json::json;

/// A shim with the old name: tests supply JSON, the code reads the selection.
fn ring_from_json(value: &serde_json::Value) -> Ring {
    ring_of(&crate::gui::tray::app::snapshot::TraySnapshot::from_value(
        value,
    ))
}

/// The roles one snapshot can show at once (an unavailable route, which yields `ExitDown`, has no active exit).
const ALL_ROLES: [RingRole; 4] = [
    RingRole::ActiveExit,
    RingRole::ExitClient,
    RingRole::Online,
    RingRole::Offline,
];

/// Roles in sector order.
fn roles_of(ring: Ring) -> Vec<RingRole> {
    (0..ring.len()).map(|k| ring.role_at(k)).collect()
}

fn peer(id: &str, online: bool) -> serde_json::Value {
    json!({ "id": id, "id_full": id, "self": false, "online": online })
}

fn me(id: &str) -> serde_json::Value {
    json!({ "id": id, "id_full": id, "self": true, "online": true })
}

fn state(peers: Vec<serde_json::Value>, route: serde_json::Value) -> serde_json::Value {
    json!({ "node": { "status": "online" }, "peers": peers, "route": route })
}

fn exit_route(exits: serde_json::Value) -> serde_json::Value {
    json!({ "state": "exit", "exits": exits })
}

fn chosen(id: &str, active: Option<bool>) -> serde_json::Value {
    match active {
        Some(a) => json!({ "id": id, "id_full": id, "online": true, "exit": true, "active": a }),
        None => json!({ "id": id, "id_full": id, "online": true, "exit": true }),
    }
}

/// A snapshot with `exit: { enabled, clients }` — the "exits through us" flag (`wt/exit-clients`).
fn with_clients(mut snapshot: serde_json::Value, ids: &[&str]) -> serde_json::Value {
    snapshot["exit"] = json!({ "enabled": true, "domains": 3, "clients": ids });
    snapshot
}

fn direct(peers: Vec<serde_json::Value>) -> serde_json::Value {
    state(peers, json!({ "state": "direct", "exits": [] }))
}

mod ring_from_json_parse {
    use super::*;

    #[test]
    fn active_exit_is_via_others_by_online() {
        let peers = vec![
            me("idS"),
            peer("idA", true),
            peer("idB", true),
            peer("idC", false),
        ];
        let exits = json!([chosen("idA", Some(false)), chosen("idB", Some(true))]);
        let ring = ring_from_json(&state(peers, exit_route(exits)));
        assert_eq!(
            roles_of(ring),
            [RingRole::ActiveExit, RingRole::Online, RingRole::Offline]
        );
    }

    #[test]
    fn no_active_flag_takes_first_exit() {
        let peers = vec![peer("idA", true), peer("idB", true)];
        let exits = json!([chosen("idA", None), chosen("idB", None)]);
        let ring = ring_from_json(&state(peers, exit_route(exits)));
        assert_eq!(roles_of(ring), [RingRole::ActiveExit, RingRole::Online]);
    }

    #[test]
    fn direct_route_has_no_via_even_with_exits() {
        let peers = vec![peer("idA", true), peer("idB", true)];
        let route = json!({ "state": "direct", "exits": [chosen("idA", Some(true))] });
        let ring = ring_from_json(&state(peers, route));
        assert_eq!(roles_of(ring), [RingRole::Online, RingRole::Online]);
    }

    #[test]
    fn offline_peer_is_offline() {
        let ring = ring_from_json(&direct(vec![peer("idA", false)]));
        assert_eq!(roles_of(ring), [RingRole::Offline]);
    }

    #[test]
    fn self_peer_is_left_out() {
        let ring = ring_from_json(&direct(vec![
            me("idS"),
            peer("idA", true),
            peer("idB", true),
        ]));
        assert_eq!(ring.len(), 2);
    }

    #[test]
    fn exit_id_matching_no_peer_is_added_as_via() {
        // Earlier the test expected "no blue"; now the active exit is always drawn (view_ring_missing_tests).
        let peers = vec![peer("idA", true), peer("idB", true)];
        let exits = json!([chosen("idZ", Some(true))]);
        let ring = ring_from_json(&state(peers, exit_route(exits)));
        assert_eq!(
            roles_of(ring),
            [RingRole::ActiveExit, RingRole::Online, RingRole::Online]
        );
    }

    #[test]
    fn missing_or_wrong_peers_is_empty() {
        assert_eq!(ring_from_json(&json!({})), Ring::EMPTY);
        assert_eq!(ring_from_json(&json!({ "peers": "x" })), Ring::EMPTY);
        assert_eq!(ring_from_json(&json!({ "peers": null })), Ring::EMPTY);
        assert_eq!(ring_from_json(&json!({ "peers": [] })), Ring::EMPTY);
        assert_eq!(ring_from_json(&json!(null)), Ring::EMPTY);
    }

    #[test]
    fn peer_without_online_is_offline() {
        let bare = json!({ "id": "idA", "id_full": "idA", "self": false });
        let ring = ring_from_json(&direct(vec![bare]));
        assert_eq!(roles_of(ring), [RingRole::Offline]);
    }

    #[test]
    fn peer_without_id_full_does_not_panic() {
        let no_id = json!({ "self": false, "online": true });
        let exits = json!([chosen("idA", Some(true))]);
        let ring = ring_from_json(&state(vec![no_id], exit_route(exits)));
        // A peer without `id_full` does not become an exit (it stays "online"); the exit idA is not found among the peers,
        // so a blue sector is added (view_ring_missing_tests).
        assert_eq!(roles_of(ring), [RingRole::ActiveExit, RingRole::Online]);
    }

    #[test]
    fn peer_order_in_snapshot_does_not_matter() {
        let (a, b, c) = (peer("idA", true), peer("idB", false), peer("idC", true));
        let route = json!({ "state": "direct", "exits": [] });
        let one = state(vec![a.clone(), b.clone(), c.clone()], route.clone());
        let two = state(vec![c, a, b], route);
        assert_eq!(ring_from_json(&one), ring_from_json(&two));
    }
}

mod exit_clients_parse {
    use super::*;

    fn three() -> serde_json::Value {
        direct(vec![
            peer("idA", true),
            peer("idB", true),
            peer("idC", true),
        ])
    }

    #[test]
    fn listed_peer_is_exit_client() {
        let ring = ring_from_json(&with_clients(three(), &["idB"]));
        assert_eq!(
            roles_of(ring),
            [RingRole::ExitClient, RingRole::Online, RingRole::Online]
        );
    }

    #[test]
    fn blue_purple_green_grey_in_order() {
        let peers = vec![
            peer("idD", false),
            peer("idC", true),
            peer("idB", true),
            peer("idA", true),
        ];
        let exits = json!([chosen("idA", Some(true))]);
        let snapshot = with_clients(state(peers, exit_route(exits)), &["idB"]);
        assert_eq!(roles_of(ring_from_json(&snapshot)), ALL_ROLES);
    }

    #[test]
    fn unavailable_route_marks_selected_exits_down() {
        let peers = vec![peer("idA", true), peer("idB", true)];
        let route = json!({ "state": "unavailable", "exits": [chosen("idA", None)] });
        let ring = ring_from_json(&state(peers, route));
        assert_eq!(roles_of(ring), [RingRole::ExitDown, RingRole::Online]);
    }

    #[test]
    fn exit_and_client_at_once_is_exit_via() {
        let exits = json!([chosen("idA", Some(true))]);
        let snapshot = state(vec![peer("idA", true)], exit_route(exits));
        let ring = ring_from_json(&with_clients(snapshot, &["idA"]));
        assert_eq!(roles_of(ring), [RingRole::ActiveExit]);
    }

    #[test]
    fn offline_client_is_offline() {
        let snapshot = direct(vec![peer("idA", false), peer("idB", true)]);
        let ring = ring_from_json(&with_clients(snapshot, &["idA"]));
        assert_eq!(roles_of(ring), [RingRole::Online, RingRole::Offline]);
    }

    #[test]
    fn offline_active_exit_is_offline() {
        let exits = json!([chosen("idA", Some(true))]);
        let snapshot = state(vec![peer("idA", false)], exit_route(exits));
        assert_eq!(roles_of(ring_from_json(&snapshot)), [RingRole::Offline]);
    }

    #[test]
    fn disabled_exit_with_empty_list_has_no_purple() {
        let mut snapshot = three();
        snapshot["exit"] = json!({ "enabled": false, "domains": 0, "clients": [] });
        let ring = ring_from_json(&snapshot);
        assert_eq!(ring.count(RingRole::ExitClient), 0);
        assert_eq!(ring.count(RingRole::Online), 3);
    }

    #[test]
    fn snapshot_without_exit_or_clients_has_no_purple() {
        let mut no_clients = three();
        no_clients["exit"] = json!({ "enabled": true, "domains": 1 });
        for snapshot in [three(), no_clients] {
            let ring = ring_from_json(&snapshot);
            assert_eq!(ring.count(RingRole::ExitClient), 0);
            assert_eq!(ring.len(), 3);
        }
    }

    #[test]
    fn clients_not_an_array_or_not_strings_is_empty_list() {
        for clients in [json!("idB"), json!(7), json!(null), json!([5, null, {}])] {
            let mut snapshot = three();
            snapshot["exit"] = json!({ "enabled": true, "clients": clients });
            let ring = ring_from_json(&snapshot);
            assert_eq!(ring.count(RingRole::ExitClient), 0, "{clients}");
            assert_eq!(ring.count(RingRole::Online), 3, "{clients}");
        }
    }
}

/// An end-to-end link "snapshot → ring": the JSON comes from a real `NodeSnapshot` through `serde_json`,
/// as in `/api/state` (`web/routes/state.rs`), not by hand. Renaming a field or changing the type of
/// `exit.clients`, `peers[].id_full`, `peers[].self`, `peers[].online` in the snapshot breaks these tests.
mod ring_from_real_snapshot {
    use super::*;
    use crate::web_gui::{ExitInfo, NodeSnapshot, PeerInfo};

    const ME: &str = "aa00bb11cc22dd33";
    const A: &str = "a1a1a1a1a1a1a1a1";
    const B: &str = "b2b2b2b2b2b2b2b2";
    const C: &str = "c3c3c3c3c3c3c3c3";

    /// A member with a short `id` (the first 4 characters) that differs from the full `id_full`: the ring must
    /// compare the full one.
    fn member(id_full: &str, is_self: bool) -> PeerInfo {
        PeerInfo {
            id: id_full.chars().take(4).collect(),
            id_full: id_full.to_string(),
            is_self,
            online: true,
            ..Default::default()
        }
    }

    fn snapshot_with(clients: &[&str]) -> NodeSnapshot {
        NodeSnapshot {
            exit: ExitInfo {
                enabled: true,
                domains: 2,
                clients: clients.iter().map(|id| id.to_string()).collect(),
            },
            peers: vec![
                member(ME, true),
                member(A, false),
                member(B, false),
                member(C, false),
            ],
            ..Default::default()
        }
    }

    /// The snapshot as the tray receives it from `/api/state`: `serde_json` to bytes and back.
    fn over_the_wire(snapshot: &NodeSnapshot) -> serde_json::Value {
        let bytes = serde_json::to_vec(snapshot).expect("the snapshot serializes");
        serde_json::from_slice(&bytes).expect("the response parses as JSON")
    }

    #[test]
    fn serialized_clients_become_purple_sectors() {
        let state = over_the_wire(&snapshot_with(&[A, C]));
        assert!(
            state["exit"]["clients"].is_array(),
            "the snapshot lost exit.clients as an array"
        );
        let ring = ring_from_json(&state);
        assert_eq!(
            ring.len(),
            3,
            "the node itself is not in the ring, the other three are"
        );
        assert_eq!(
            ring.count(RingRole::ExitClient),
            2,
            "A and C from exit.clients are violet"
        );
        assert_eq!(
            ring.count(RingRole::Online),
            1,
            "B is outside the list — just online"
        );
        assert_eq!(
            roles_of(ring),
            [RingRole::ExitClient, RingRole::ExitClient, RingRole::Online]
        );
    }

    #[test]
    fn serialized_peer_outside_clients_is_not_purple() {
        let state = over_the_wire(&snapshot_with(&[B]));
        let ring = ring_from_json(&state);
        assert_eq!(
            ring.count(RingRole::ExitClient),
            1,
            "only B from exit.clients"
        );
        assert_eq!(
            ring.count(RingRole::Online),
            2,
            "A and C are outside the list — online"
        );
    }

    #[test]
    fn serialized_empty_clients_have_no_purple() {
        let state = over_the_wire(&snapshot_with(&[]));
        assert_eq!(
            state["exit"]["clients"],
            json!([]),
            "the field is always there, an empty array"
        );
        let ring = ring_from_json(&state);
        assert_eq!(ring.count(RingRole::ExitClient), 0);
        assert_eq!(ring.count(RingRole::Online), 3);
    }
}
