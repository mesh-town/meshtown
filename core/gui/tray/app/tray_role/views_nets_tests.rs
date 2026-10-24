//! Tests of the tray view built from the summary of all networks: which network the flat menu shows, when the menu is
//! nested, and how the icon color and the link follow the networks.

use super::super::{snapshot_view, snapshot_view_nets, views_from_state_json};
use crate::gui::tray::app::exit_menu::ExitMenu;
use crate::gui::tray::app::local_api::stub_node::{
    friends, overview_json, state_json, NetSpec, StateSpec,
};
use crate::gui::tray::app::nets_rows::{exit_rows, RowKind};
use crate::gui::tray::app::proxy_state::ProxyState;
use crate::gui::tray::app::snapshot::TraySnapshot;
use crate::gui::tray::app::tray_texts::net_none_status_text;
use crate::gui::tray::app::view::{node_status_menu_line, NetLink, RouteLine, TrayColor, View};
use serde_json::Value;

mod nothing_to_show {
    use super::*;

    #[test]
    fn view_and_menu_are_those_of_the_single_state() {
        // given
        let keyless = NetSpec::keyless("main");
        let single = keyless.state.clone().unwrap();

        // when
        let (menu, view) = shown(&[keyless]);

        // then
        assert_eq!(
            (menu.clone(), view),
            views_from_state_json(&single, &plan())
        );
        assert_eq!(menu.slot, None);
        assert_eq!(view.net, NetLink::NoNetwork);
        assert_eq!(view.color, TrayColor::Grey);
    }

    #[test]
    fn status_line_says_no_network_joined() {
        // given
        let (menu, view) = shown(&[NetSpec::keyless("main")]);

        // when
        let line = node_status_menu_line(view, &menu.route);

        // then
        assert_eq!(line, net_none_status_text());
    }

    #[test]
    fn main_slot_without_state_gives_way_to_the_first_slot_with_one() {
        // given
        let serving = StateSpec {
            network: false,
            serves_exit: true,
            ..Default::default()
        };
        let second = NetSpec {
            id: "ab12cd",
            state: Some(state_json(&serving)),
            ..NetSpec::keyless("ab12cd")
        };

        // when
        let (menu, _) = shown(&[NetSpec::broken("main"), second]);

        // then
        assert!(menu.serves_exit, "the state of the second slot is shown");
        assert_eq!(menu.slot, None);
    }
}

mod one_network_without_a_key_next_to_an_added_one {
    use super::*;

    #[test]
    fn menu_is_flat_and_shows_the_added_network() {
        // given
        let keyless = proxy_carrier();

        // when
        let (menu, _) = shown(&[keyless, super::work()]);

        // then
        assert_eq!(menu.slot.as_deref(), Some("ab12cd"));
        assert!(menu.nets.is_empty() && menu.groups.is_empty());
        assert_eq!(menu.items.len(), 1);
    }

    #[test]
    fn proxy_line_comes_from_the_network_that_carries_the_proxy() {
        // given
        let keyless = proxy_carrier();

        // when
        let (menu, _) = shown(&[keyless, super::work()]);

        // then
        assert_eq!(menu.proxy, ProxyState::On("127.0.0.1:1080".to_string()));
    }
}

mod two_networks {
    use super::*;

    #[test]
    fn menu_is_nested_and_has_no_flat_exits() {
        // given / when
        let (menu, _) = shown(&[friends(), super::work()]);

        // then
        assert_eq!(menu.nets.len(), 2);
        assert_eq!(menu.groups.len(), 2);
        assert!(menu.items.is_empty());
        assert_eq!(menu.slot, None);
    }

    #[test]
    fn exits_are_titled_per_network() {
        // given
        let (menu, _) = shown(&[friends(), super::work()]);

        // when
        let rows = exit_rows(&menu);

        // then
        let headers = rows.iter().filter(|r| r.kind == RowKind::Header).count();
        assert_eq!(headers, 2);
    }

    #[test]
    fn status_line_names_the_network_of_the_exit() {
        // given
        let work = NetSpec {
            proxy: true,
            ..super::work()
        };
        let main = NetSpec {
            proxy: false,
            ..friends()
        };

        // when
        let (menu, view) = shown(&[main, work]);

        // then
        assert_eq!(menu.route, RouteLine::Via("peer-b (work)".to_string()));
        assert!(node_status_menu_line(view, &menu.route).ends_with("via peer-b (work)"));
    }

    #[test]
    fn color_is_not_grey_when_only_the_second_network_is_online() {
        // given
        let offline = NetSpec {
            status: "offline",
            state: Some(state_json(&StateSpec {
                status: "offline",
                ..Default::default()
            })),
            ..friends()
        };

        // when
        let (_, view) = shown(&[offline, super::work()]);

        // then
        assert_ne!(view.color, TrayColor::Grey);
        assert_eq!(view.net, NetLink::Joined);
    }

    #[test]
    fn color_is_grey_when_every_network_is_offline() {
        // given
        let offline = |net: NetSpec<'static>| NetSpec {
            status: "offline",
            state: Some(state_json(&StateSpec {
                status: "offline",
                ..Default::default()
            })),
            ..net
        };

        // when
        let (_, view) = shown(&[offline(friends()), offline(super::work())]);

        // then
        assert_eq!(view.color, TrayColor::Grey);
        assert_eq!(view.net, NetLink::Lost);
    }
}

mod one_network {
    use super::*;

    #[test]
    fn view_is_the_same_as_for_the_single_state() {
        // given
        let net = friends();
        let single = net.state.clone().unwrap();

        // when
        let (menu, view) = shown(&[net]);

        // then
        let (single_menu, single_view) = views_from_state_json(&single, &plan());
        assert_eq!(view, single_view);
        assert_eq!(ExitMenu { slot: None, ..menu }, single_menu);
    }

    #[test]
    fn exit_rows_are_flat_with_plain_ids() {
        // given
        let (menu, _) = shown(&[friends()]);

        // when
        let rows = exit_rows(&menu);

        // then
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["exit:peer-one"]);
        assert!(rows.iter().all(|r| r.kind == RowKind::Check));
    }

    #[test]
    fn status_line_has_no_network_name() {
        // given
        let (menu, _) = shown(&[super::work()]);

        // then
        assert_eq!(menu.route, RouteLine::Via("peer-b".to_string()));
    }
}

mod snapshot_view_nets {
    use super::*;

    #[test]
    fn any_online_lifts_an_offline_snapshot() {
        // given
        let snap = offline_snapshot();

        // when
        let view = snapshot_view_nets(&snap, true);

        // then
        assert_ne!(view.color, TrayColor::Grey);
        assert_eq!(view.net, NetLink::Joined);
    }

    #[test]
    fn without_any_online_the_snapshot_decides() {
        // given
        let snap = offline_snapshot();

        // when
        let view = snapshot_view_nets(&snap, false);

        // then
        assert_eq!(view.color, TrayColor::Grey);
        assert_eq!(view.net, NetLink::Lost);
        assert_eq!(snapshot_view(&snap), view, "the plain view is the same");
    }
}

/// A main network without a key that carries the proxy.
fn proxy_carrier() -> NetSpec<'static> {
    NetSpec {
        proxy: true,
        state: Some(state_json(&StateSpec {
            network: false,
            socks: Some("127.0.0.1:1080"),
            ..Default::default()
        })),
        ..NetSpec::keyless("main")
    }
}

mod flat_menu_of_a_managed_network {
    use super::*;
    use serde_json::json;

    #[test]
    fn member_cannot_invite() {
        // given
        let json =
            summary_with(json!({ "kind": "Managed", "me_admin": false, "can_invite": false }));

        // when
        let (menu, _) = views_from_state_json(&json, &plan());

        // then
        assert!(!menu.can_invite, "the flat invite item must be grey");
    }

    #[test]
    fn admin_can_invite() {
        let json = summary_with(json!({ "kind": "Managed", "me_admin": true, "can_invite": true }));

        let (menu, _) = views_from_state_json(&json, &plan());

        assert!(menu.can_invite);
    }

    #[test]
    fn peer_network_can_invite() {
        let (menu, _) = shown(&[friends()]);

        assert!(menu.can_invite);
    }

    #[test]
    fn single_state_reply_can_invite() {
        let single = state_json(&StateSpec::default());

        let (menu, _) = views_from_state_json(&single, &plan());

        assert!(menu.can_invite);
    }
}

mod deleted_network_notes {
    use super::*;
    use serde_json::json;

    #[test]
    fn menu_carries_the_notes_of_the_summary() {
        // given
        let mut json = overview_json(&[friends()]);
        json["notices"] = json!([{ "id": 4, "kind": "net_deleted", "label": "work" }]);

        // when
        let (menu, _) = views_from_state_json(&json, &plan());

        // then
        let notes: Vec<(u64, &str)> = menu
            .notices
            .iter()
            .map(|n| (n.id, n.label.as_str()))
            .collect();
        assert_eq!(notes, [(4, "work")]);
    }

    #[test]
    fn nested_menu_carries_them_too() {
        let mut json = overview_json(&[friends(), work()]);
        json["notices"] = json!([{ "id": 4, "kind": "net_deleted", "label": "home" }]);

        let (menu, _) = views_from_state_json(&json, &plan());

        assert_eq!(menu.nets.len(), 2);
        assert_eq!(menu.notices.len(), 1);
    }

    #[test]
    fn menu_without_notices_has_none() {
        let (menu, _) = shown(&[friends()]);

        assert!(menu.notices.is_empty());
    }

    #[test]
    fn single_state_reply_has_none() {
        let single = state_json(&StateSpec::default());

        let (menu, _) = views_from_state_json(&single, &plan());

        assert!(menu.notices.is_empty());
    }
}

fn plan() -> ProxyState {
    ProxyState::On("127.0.0.1:1080".to_string())
}

fn work() -> NetSpec<'static> {
    crate::gui::tray::app::local_api::stub_node::work()
}

/// The menu and the view for a node that holds `nets`.
fn shown(nets: &[NetSpec]) -> (ExitMenu, View) {
    let summary: Value = overview_json(nets);
    views_from_state_json(&summary, &plan())
}

fn offline_snapshot() -> TraySnapshot {
    let state = StateSpec {
        status: "offline",
        ..Default::default()
    };
    TraySnapshot::from_value(&state_json(&state))
}

/// A one-network summary whose entry has `extra` fields merged in.
fn summary_with(extra: Value) -> Value {
    let mut json = overview_json(&[friends()]);
    for (key, value) in extra.as_object().expect("an object") {
        json["nets"][0][key] = value.clone();
    }
    json
}
