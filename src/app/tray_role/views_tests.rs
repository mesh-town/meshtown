//! Tests for `snapshot_view`: the member ring is counted for any icon color.

use super::*;
use crate::app::proxy_state::{ProxyOff, ProxyState};
use crate::app::view::NetLink;
use crate::app::view_ring::RingRole;
use serde_json::json;

/// A shim with the old name: the tests supply JSON, and the code reads a sample.
fn snapshot_view(value: &serde_json::Value) -> View {
    super::snapshot_view(&crate::app::snapshot::TraySnapshot::from_value(value))
}

fn peer(id: &str, online: bool) -> serde_json::Value {
    json!({ "id": id, "id_full": id, "self": false, "online": online })
}

/// A node snapshot: status, route state and peers.
fn snapshot(status: &str, route: &str, peers: Vec<serde_json::Value>) -> serde_json::Value {
    json!({ "node": { "status": status }, "peers": peers, "route": { "state": route } })
}

#[test]
fn grey_snapshot_with_peers_keeps_ring() {
    let view = snapshot_view(&snapshot(
        "offline",
        "direct",
        vec![peer("a", true), peer("b", false)],
    ));
    assert_eq!(view.color, TrayColor::Grey);
    assert_eq!(view.ring.count(RingRole::Online), 1);
    assert_eq!(view.ring.count(RingRole::Offline), 1);
    assert_eq!(
        view.net,
        NetLink::Lost,
        "a snapshot without a network key — the network exists"
    );
}

/// A snapshot with our own network (`network` is not `null`).
fn with_network(status: &str, peers: Vec<serde_json::Value>) -> serde_json::Value {
    let mut state = snapshot(status, "direct", peers);
    state["network"] = json!({ "name": "x" });
    state
}

#[test]
fn grey_starting_with_network_is_connecting() {
    let view = snapshot_view(&with_network("starting", vec![peer("a", true)]));
    assert_eq!(view.net, NetLink::Connecting);
    assert_eq!(view.color, TrayColor::Grey);
    assert_eq!(view.ring.count(RingRole::Online), 1);
}

#[test]
fn peers_change_changes_grey_view() {
    let one = snapshot_view(&with_network("starting", vec![peer("a", true)]));
    let other = snapshot_view(&with_network(
        "starting",
        vec![peer("b", false), peer("c", true)],
    ));
    assert_ne!(one, other, "network membership with a grey icon is visible in the ring");
}

#[test]
fn offline_node_with_offline_peers_draws_grey_sectors() {
    let peers = vec![peer("a", false), peer("b", false)];
    let view = snapshot_view(&snapshot("offline", "direct", peers));
    assert_eq!(view.color, TrayColor::Grey);
    assert_eq!(view.ring.count(RingRole::Offline), 2);
    assert_eq!(view.ring.len(), 2);
}

#[test]
fn status_change_changes_grey_view() {
    let starting = snapshot_view(&with_network("starting", vec![peer("a", true)]));
    let offline = snapshot_view(&with_network("offline", vec![peer("a", true)]));
    assert_ne!(starting, offline);
}

#[test]
fn red_snapshots_with_different_peer_online_differ() {
    let with = |online| snapshot_view(&snapshot("online", "unavailable", vec![peer("a", online)]));
    let (up, down) = (with(true), with(false));
    assert_eq!(up.color, TrayColor::Red);
    assert_eq!(up.ring.count(RingRole::Online), 1);
    assert_eq!(down.ring.count(RingRole::Offline), 1);
    assert_ne!(up, down, "network membership with a red icon is visible in the ring");
}

#[test]
fn green_ring_snapshots_with_different_composition_differ() {
    let with = |online| snapshot_view(&snapshot("online", "direct", vec![peer("a", online)]));
    let (up, down) = (with(true), with(false));
    assert_eq!(up.color, TrayColor::GreenOutline);
    assert_eq!(up.ring.count(RingRole::Online), 1);
    assert_eq!(down.ring.count(RingRole::Offline), 1);
    assert_ne!(up, down);
}

#[test]
fn green_snapshot_via_exit_keeps_the_ring() {
    let view = snapshot_view(&snapshot(
        "online",
        "exit",
        vec![peer("a", true), peer("b", true)],
    ));
    assert_eq!(view.color, TrayColor::Green);
    assert_eq!(view.ring.count(RingRole::Online), 2);
}

/// A snapshot of a live node with a proxy: on or off (the address in `socks.listen` is always there).
fn with_socks(enabled: bool) -> serde_json::Value {
    let mut state = snapshot("online", "direct", vec![]);
    state["socks"] = json!({ "enabled": enabled, "listen": "127.0.0.1:1080" });
    state
}

#[test]
fn live_node_with_proxy_off_takes_flag_from_own() {
    let own = ProxyState::Off(ProxyOff::NoSocksFlag);
    let (menu, _) = views_from_state_json(&with_socks(false), &own);
    assert_eq!(menu.proxy, ProxyState::Off(ProxyOff::NoSocksFlag));
}

#[test]
fn live_node_with_proxy_off_without_own_flag_is_settings() {
    let own = ProxyState::Off(ProxyOff::DisabledInSettings);
    let (menu, _) = views_from_state_json(&with_socks(false), &own);
    assert_eq!(menu.proxy, ProxyState::Off(ProxyOff::DisabledInSettings));
}

#[test]
fn live_node_with_proxy_on_ignores_own_flag() {
    let own = ProxyState::Off(ProxyOff::NoSocksFlag);
    let (menu, _) = views_from_state_json(&with_socks(true), &own);
    assert_eq!(menu.proxy, ProxyState::On("127.0.0.1:1080".to_string()));
}

#[test]
fn view_carries_disk_from_snapshot() {
    use crate::app::view_disk::Disk;
    let mut busy = peer("a", true);
    busy["through_peer_bps"] = json!(1_000_000);
    let view = snapshot_view(&snapshot("online", "direct", vec![busy]));
    assert_eq!(view.disk, Some(Disk { step: 4, blue_twelfths: 12 }));
}
