//! Tests of `ring_from_json`: an active exit that is not among `peers` gives one blue sector.

use super::*;
use serde_json::json;

/// A shim with the old name: tests supply JSON, the code reads the selection.
fn ring_from_json(value: &serde_json::Value) -> Ring {
    ring_of(&crate::app::snapshot::TraySnapshot::from_value(value))
}

fn peer(id: &str, online: bool) -> serde_json::Value {
    json!({ "id": id, "id_full": id, "self": false, "online": online })
}

fn me(id: &str) -> serde_json::Value {
    json!({ "id": id, "id_full": id, "self": true, "online": true })
}

fn exit_entry(id: &str, active: bool) -> serde_json::Value {
    json!({ "id": id, "id_full": id, "online": true, "exit": true, "active": active })
}

fn exit_state(peers: Vec<serde_json::Value>, exits: Vec<serde_json::Value>) -> serde_json::Value {
    let route = json!({ "state": "exit", "exits": exits });
    json!({ "node": { "status": "online" }, "peers": peers, "route": route })
}

fn roles_of(ring: Ring) -> Vec<RingRole> {
    (0..ring.len()).map(|k| ring.role_at(k)).collect()
}

#[test]
fn lost_exit_is_added_as_first_blue_sector() {
    let peers = vec![me("idM"), peer("idA", true), peer("idB", true)];
    let ring = ring_from_json(&exit_state(peers, vec![exit_entry("idZ", true)]));
    assert_eq!(ring.role_at(0), RingRole::ActiveExit);
    assert_eq!(ring.len(), 3, "peers except itself (2) and the lost exit");
    assert_eq!(roles_of(ring), [RingRole::ActiveExit, RingRole::Online, RingRole::Online]);
}

#[test]
fn exit_found_among_peers_is_not_duplicated() {
    let peers = vec![peer("idA", true), peer("idB", true)];
    let ring = ring_from_json(&exit_state(peers, vec![exit_entry("idA", true)]));
    assert_eq!(roles_of(ring), [RingRole::ActiveExit, RingRole::Online]);
}

#[test]
fn offline_exit_found_among_peers_stays_grey_without_duplicate() {
    let peers = vec![peer("idA", false), peer("idB", true)];
    let ring = ring_from_json(&exit_state(peers, vec![exit_entry("idA", true)]));
    assert_eq!(roles_of(ring), [RingRole::Online, RingRole::Offline]);
}

#[test]
fn exit_that_is_this_node_is_not_added() {
    let peers = vec![me("idM"), peer("idA", true)];
    let ring = ring_from_json(&exit_state(peers, vec![exit_entry("idM", true)]));
    assert_eq!(roles_of(ring), [RingRole::Online]);
}

#[test]
fn only_the_active_lost_exit_gets_a_sector() {
    let peers = vec![peer("idA", true)];
    let exits = vec![exit_entry("idY", false), exit_entry("idZ", true)];
    let ring = ring_from_json(&exit_state(peers, exits));
    assert_eq!(roles_of(ring), [RingRole::ActiveExit, RingRole::Online]);
}

#[test]
fn direct_route_with_exits_has_no_blue() {
    let mut state = exit_state(vec![peer("idA", true)], vec![exit_entry("idZ", true)]);
    state["route"]["state"] = json!("direct");
    assert_eq!(roles_of(ring_from_json(&state)), [RingRole::Online]);
}

#[test]
fn peers_not_an_array_stays_empty_even_with_active_exit() {
    let mut state = exit_state(vec![], vec![exit_entry("idZ", true)]);
    state["peers"] = json!("x");
    assert_eq!(ring_from_json(&state), Ring::EMPTY);
    state.as_object_mut().unwrap().remove("peers");
    assert_eq!(ring_from_json(&state), Ring::EMPTY);
}

#[test]
fn snapshot_without_route_is_unchanged() {
    let state = json!({ "peers": [peer("idA", true), peer("idB", false)] });
    assert_eq!(
        roles_of(ring_from_json(&state)),
        [RingRole::Online, RingRole::Offline]
    );
}

#[test]
fn non_object_peer_counts_offline() {
    let state = json!({ "peers": [5] });
    assert_eq!(roles_of(ring_from_json(&state)), [RingRole::Offline]);
}

#[test]
fn peers_absent_vs_empty_with_active_exit() {
    let mut state = exit_state(vec![], vec![exit_entry("idZ", true)]);
    state["peers"] = json!("x");
    assert_eq!(ring_from_json(&state), Ring::EMPTY);
    state["peers"] = json!([]);
    assert_eq!(roles_of(ring_from_json(&state)), [RingRole::ActiveExit]);
}

fn route_state(
    state: &str,
    peers: Vec<serde_json::Value>,
    exits: Vec<serde_json::Value>,
) -> serde_json::Value {
    let route = json!({ "state": state, "exits": exits });
    json!({ "node": { "status": "online" }, "peers": peers, "route": route })
}

fn exit_off(id: &str) -> serde_json::Value {
    json!({ "id": id, "id_full": id, "online": false, "exit": true, "active": false })
}

#[test]
fn unavailable_exit_in_peers_offline_is_exit_down() {
    let peers = vec![me("idM"), peer("idA", true), peer("idX", false)];
    let state = route_state("unavailable", peers, vec![exit_off("idX")]);
    assert_eq!(roles_of(ring_from_json(&state)), [RingRole::ExitDown, RingRole::Online]);
}

#[test]
fn unavailable_exit_missing_in_peers_is_one_exit_down() {
    let peers = vec![me("idM"), peer("idA", true)];
    let state = route_state("unavailable", peers, vec![exit_off("idX")]);
    assert_eq!(roles_of(ring_from_json(&state)), [RingRole::ExitDown, RingRole::Online]);
}

#[test]
fn two_unavailable_exits_missing_in_peers_are_two_exit_down() {
    let peers = vec![me("idM")];
    let exits = vec![exit_off("idX"), exit_off("idY")];
    let state = route_state("unavailable", peers, exits);
    assert_eq!(roles_of(ring_from_json(&state)), [RingRole::ExitDown, RingRole::ExitDown]);
}

#[test]
fn exit_state_backup_exit_offline_stays_grey() {
    let peers = vec![me("idM"), peer("idA", true), peer("idB", false)];
    let exits = vec![exit_entry("idA", true), exit_off("idB")];
    let state = route_state("exit", peers, exits);
    assert_eq!(roles_of(ring_from_json(&state)), [RingRole::ActiveExit, RingRole::Offline]);
}

#[test]
fn direct_route_has_no_exit_down() {
    let peers = vec![me("idM"), peer("idB", false)];
    let state = route_state("direct", peers, vec![exit_off("idB")]);
    assert_eq!(roles_of(ring_from_json(&state)), [RingRole::Offline]);
}
