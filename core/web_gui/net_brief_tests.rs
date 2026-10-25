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
            admission_refused: true,
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
    assert!(b.admission_refused);
}

#[test]
fn the_brief_carries_the_refusal() {
    // given: a snapshot whose members block says this computer is kept out
    let host = FakeHost::new(vec![handle("main", rich_snapshot(), None)]);

    // when
    let b = brief_of(&host.main_slot(), &NetSlotId::main());

    // then
    assert!(b.admission_refused, "the flag is taken from the snapshot");
}

#[test]
fn a_slot_without_a_key_has_no_label_and_no_id() {
    // given
    let host = FakeHost::new(vec![handle("main", rich_snapshot(), None)]);

    // when
    let b = brief_of(&host.main_slot(), &NetSlotId::main());

    // then
    assert_eq!((b.label, b.id_hex), (None, None));
}

mod proxy {
    use std::sync::Arc;

    use super::*;

    mod net_is_the_default_main {
        use super::*;

        #[test]
        fn only_the_main_slot_has_the_proxy() {
            // given
            let host = setup(&["main", "ab2c3d", "cd4e5f"], None, &[]);

            // when
            let got = proxies(host.as_ref());

            // then
            assert_eq!(
                got,
                flags(&[("main", true), ("ab2c3d", false), ("cd4e5f", false)])
            );
        }
    }

    mod net_names_a_second_slot {
        use super::*;

        #[test]
        fn only_that_slot_has_the_proxy_and_main_does_not() {
            // given
            let host = setup(&["main", "ab2c3d", "cd4e5f"], Some("ab2c3d"), &[]);

            // when
            let got = proxies(host.as_ref());

            // then
            assert_eq!(
                got,
                flags(&[("main", false), ("ab2c3d", true), ("cd4e5f", false)])
            );
        }

        #[test]
        fn naming_the_last_slot_marks_exactly_that_one() {
            // given
            let host = setup(&["main", "ab2c3d", "cd4e5f"], Some("cd4e5f"), &[]);

            // when
            let got = proxies(host.as_ref());

            // then
            assert_eq!(
                got,
                flags(&[("main", false), ("ab2c3d", false), ("cd4e5f", true)])
            );
        }
    }

    mod net_names_a_slot_that_is_not_loaded {
        use super::*;

        #[test]
        fn no_slot_has_the_proxy() {
            // given
            let host = setup(&["main", "ab2c3d"], Some("ee2222"), &[]);

            // when
            let got = proxies(host.as_ref());

            // then
            assert_eq!(got, flags(&[("main", false), ("ab2c3d", false)]));
        }
    }

    mod net_names_a_broken_slot {
        use super::*;

        #[test]
        fn the_broken_brief_has_no_proxy_and_the_live_ones_neither() {
            // given
            let host = setup(&["main", "ab2c3d"], Some("ee2222"), &["ee2222"]);

            // when
            let got = proxies(host.as_ref());

            // then
            assert_eq!(
                got,
                flags(&[("main", false), ("ab2c3d", false), ("ee2222", false)])
            );
        }

        #[test]
        fn the_main_slot_keeps_the_proxy_next_to_a_broken_one() {
            // given
            let host = setup(&["main"], None, &["ee2222"]);

            // when
            let got = proxies(host.as_ref());

            // then
            assert_eq!(got, flags(&[("main", true), ("ee2222", false)]));
        }
    }

    fn setup(live: &[&str], proxy_net: Option<&str>, broken: &[&str]) -> Arc<FakeHost> {
        let host = FakeHost::new(
            live.iter()
                .map(|id| handle(id, rich_snapshot(), None))
                .collect(),
        );
        if let Some(id) = proxy_net {
            host.set_proxy_net(NetSlotId::parse(id).unwrap());
        }
        host.set_broken(
            broken
                .iter()
                .map(|id| NetSlotId::parse(id).unwrap())
                .collect(),
        );
        host
    }

    fn proxies(host: &FakeHost) -> Vec<(String, bool)> {
        briefs(host).into_iter().map(|b| (b.id, b.proxy)).collect()
    }

    fn flags(expected: &[(&str, bool)]) -> Vec<(String, bool)> {
        expected
            .iter()
            .map(|(id, on)| (id.to_string(), *on))
            .collect()
    }
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
    assert!(
        !b.admission_refused,
        "a broken slot never reports a refusal"
    );
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
        "id",
        "label",
        "id_hex",
        "status",
        "exit_enabled",
        "proxy",
        "route_state",
        "exits",
        "members_voting",
        "joining",
        "members_admitting",
        "me_excluded",
        "me_pending",
        "peers_online",
        "peers_total",
        "kind",
        "me_admin",
        "can_invite",
        "admission_refused",
    ];
    let at: Vec<usize> = keys
        .iter()
        .map(|k| text.find(&format!("\"{k}\":")).unwrap())
        .collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "{text}");
}

mod route_state {
    use super::*;

    mod unavailable {
        use super::*;

        #[test]
        fn every_exit_removed_reads_removed() {
            // given
            let route = route(RouteMode::Unavailable, &[true, true]);

            // when
            let word = route_word(&route);

            // then
            assert_eq!(word, "removed");
        }

        #[test]
        fn one_exit_removed_and_one_not_reads_unavailable() {
            // given
            let route = route(RouteMode::Unavailable, &[true, false]);

            // when
            let word = route_word(&route);

            // then
            assert_eq!(word, "unavailable");
        }

        #[test]
        fn no_exits_reads_unavailable() {
            // given
            let route = route(RouteMode::Unavailable, &[]);

            // when
            let word = route_word(&route);

            // then
            assert_eq!(
                word, "unavailable",
                "an empty list is not a list of removed exits"
            );
        }
    }

    mod exit {
        use super::*;

        #[test]
        fn a_removed_exit_still_reads_exit() {
            // given
            let route = route(RouteMode::Exit, &[true]);

            // when
            let word = route_word(&route);

            // then
            assert_eq!(word, "exit");
        }
    }

    // --- setup and helpers ---

    /// A route in `state` whose exits carry the given `removed` flags in order.
    fn route(state: RouteMode, removed: &[bool]) -> RouteState {
        RouteState {
            state,
            exits: removed
                .iter()
                .map(|&removed| ChosenExit {
                    removed,
                    ..ChosenExit::default()
                })
                .collect(),
        }
    }
}
