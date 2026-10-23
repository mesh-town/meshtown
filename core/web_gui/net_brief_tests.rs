use super::*;
use crate::members::network_key::NetworkKey;
use crate::web_gui::test_host::{config_with_key, slot_handle, FakeHost, SnapNode};
use crate::web_gui::{ChosenExit, MembersInfo, NodeRuntime, PeerInfo, RouteState};

fn peer(online: bool, is_self: bool) -> PeerInfo {
    PeerInfo {
        online,
        is_self,
        ..PeerInfo::default()
    }
}

fn rich_snapshot() -> crate::web_gui::NodeSnapshot {
    let mut snap = crate::web_gui::NodeSnapshot {
        node: NodeRuntime {
            status: NodeStatus::Online,
            ..NodeRuntime::default()
        },
        route: RouteState {
            state: RouteMode::Exit,
            exits: vec![ChosenExit {
                id_full: "ee".repeat(32),
                ..ChosenExit::default()
            }],
        },
        members: MembersInfo {
            voting: 4,
            admitting: 2,
            me_excluded: true,
            me_pending: true,
            ..MembersInfo::default()
        },
        peers: vec![peer(true, true), peer(true, false), peer(false, false)],
        ..Default::default()
    };
    snap.exit.enabled = true;
    snap
}

fn handle(id: &str, snap: crate::web_gui::NodeSnapshot, key: Option<&str>) -> SlotHandle {
    slot_handle(id, SnapNode::new(snap), config_with_key(key))
}

#[test]
fn fields_come_from_the_snapshot_and_the_key() {
    // given
    let key = NetworkKey::generate().unwrap().encode();
    let host = FakeHost::new(vec![handle("main", rich_snapshot(), Some(&key))]);

    // when
    let briefs = briefs(host.as_ref());

    // then
    let b = &briefs[0];
    assert_eq!(b.id, "main");
    assert!(b.id_hex.as_deref().is_some_and(|h| !h.is_empty()));
    assert_eq!(b.status, "online");
    assert!(b.exit_enabled);
    assert_eq!(b.route_state, "exit");
    assert_eq!(b.exits, vec!["ee".repeat(32)]);
    assert_eq!((b.members_voting, b.members_admitting), (4, 2));
    assert!(b.me_excluded && b.me_pending && !b.joining);
    assert_eq!((b.peers_online, b.peers_total), (1, 2));
}

#[test]
fn a_slot_without_a_key_has_no_label_and_no_id() {
    // given
    let host = FakeHost::new(vec![handle("main", rich_snapshot(), None)]);

    // when
    let b = brief_of(&host.main_slot());

    // then
    assert_eq!((b.label, b.id_hex), (None, None));
}

#[test]
fn the_proxy_belongs_to_main_only() {
    // given
    let host = FakeHost::new(vec![
        handle("main", rich_snapshot(), None),
        handle("ab2c3d", rich_snapshot(), None),
    ]);

    // when
    let proxies: Vec<bool> = briefs(host.as_ref()).iter().map(|b| b.proxy).collect();

    // then
    assert_eq!(proxies, vec![true, false]);
}

#[test]
fn the_order_is_the_order_of_slots() {
    // given
    let ids = ["main", "zz2222", "ab2c3d", "mm3333"];
    let host = FakeHost::new(
        ids.iter()
            .map(|id| handle(id, Default::default(), None))
            .collect(),
    );

    // when
    let got: Vec<String> = briefs(host.as_ref()).into_iter().map(|b| b.id).collect();

    // then
    assert_eq!(got, ids);
}

#[test]
fn a_broken_slot_comes_last_with_its_fixed_fields() {
    // given
    let host = FakeHost::new(vec![handle("main", rich_snapshot(), None)]);
    host.set_broken(vec![NetSlotId::parse("ab2c3d").unwrap()]);

    // when
    let all = briefs(host.as_ref());

    // then
    assert_eq!(all.len(), 2);
    let b = &all[1];
    assert_eq!((b.id.as_str(), b.status.as_str()), ("ab2c3d", "broken"));
    assert_eq!(b.route_state, "unavailable");
    assert!(b.label.is_none() && b.id_hex.is_none() && b.exits.is_empty());
    assert!(!b.proxy && !b.exit_enabled && !b.joining);
    assert_eq!((b.peers_total, b.members_voting), (0, 0));
}

#[test]
fn the_json_keeps_the_field_order() {
    // given
    let b = broken_brief(&NetSlotId::parse("ab2c3d").unwrap());

    // when
    let text = serde_json::to_string(&b).unwrap();

    // then
    let keys = [
        "id", "label", "id_hex", "status", "exit_enabled", "proxy", "route_state", "exits",
        "members_voting", "joining", "members_admitting", "me_excluded", "me_pending",
        "peers_online", "peers_total",
    ];
    let at: Vec<usize> = keys
        .iter()
        .map(|k| text.find(&format!("\"{k}\":")).unwrap())
        .collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "{text}");
}
