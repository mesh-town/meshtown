//! Tests of the menu rows: the "Exit via…" rows for several networks, the submenu of one network and the layout key.

use super::*;
use crate::gui::tray::app::exit_menu::disabled_exit_menu;
use crate::gui::tray::app::local_api::stub_node::{
    empty_net, friends, friends_with_peers, overview_json, state_json, work, NetSpec, StateSpec,
};
use crate::gui::tray::app::nets_menu::{groups_from_slots, nets_from_slots, NetMenu};
use crate::gui::tray::app::overview::{parse_overview, visible_slots};
use crate::gui::tray::app::proxy_state::{ProxyOff, ProxyState};
use crate::gui::tray::app::tray_texts::EMPTY_HINT;

mod exit_rows {
    use super::*;

    #[test]
    fn several_networks_get_a_header_each_with_a_separator_between() {
        // given
        let menu = menu_of(&[friends(), work()]);

        // when
        let rows = exit_rows(&menu);

        // then
        let kinds: Vec<RowKind> = rows.iter().map(|r| r.kind).collect();
        let expected = [
            RowKind::Header,
            RowKind::Check,
            RowKind::Separator,
            RowKind::Header,
            RowKind::Check,
        ];
        assert_eq!(kinds, expected, "{rows:?}");
        assert_eq!(
            (rows[0].id.as_str(), rows[0].text.as_str()),
            ("net:main", "friends")
        );
        assert!(!rows[0].enabled, "a header cannot be clicked");
    }

    #[test]
    fn exit_ids_carry_the_slot() {
        let rows = exit_rows(&menu_of(&[friends(), work()]));

        let ids: Vec<&str> = rows
            .iter()
            .filter(|r| r.kind == RowKind::Check)
            .map(|r| r.id.as_str())
            .collect();
        assert_eq!(ids, ["exit:main:peer-one", "exit:ab12cd:peerb"]);
    }

    #[test]
    fn only_the_chosen_exit_is_checked() {
        let rows = exit_rows(&menu_of(&[friends(), work()]));

        assert!(
            row(&rows, "exit:ab12cd:peerb").checked,
            "peer-b is chosen in work"
        );
        assert!(!row(&rows, "exit:main:peer-one").checked);
    }

    #[test]
    fn network_without_exits_gives_no_group() {
        let plain = empty_net("main", "friends");

        let rows = exit_rows(&menu_of(&[plain, work()]));

        let kinds: Vec<RowKind> = rows.iter().map(|r| r.kind).collect();
        assert_eq!(kinds, [RowKind::Header, RowKind::Check], "{rows:?}");
        assert_eq!(rows[0].id, "net:ab12cd");
    }

    #[test]
    fn one_network_keeps_the_plain_ids_and_has_no_header() {
        // given
        let mut menu = menu_of(&[friends()]);
        menu.items = menu.groups.remove(0).items;
        menu.nets.clear();

        // when
        let rows = exit_rows(&menu);

        // then
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["exit:peer-one"]);
        assert!(rows.iter().all(|r| r.kind != RowKind::Header));
    }

    #[test]
    fn no_exits_anywhere_shows_the_hint() {
        let menu = disabled_exit_menu(ProxyState::Off(ProxyOff::DisabledInSettings));

        let rows = exit_rows(&menu);

        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].text, EMPTY_HINT);
        assert!(!rows[0].enabled, "the hint is not a choice");
    }
}

mod empty_hint {
    use super::*;

    #[test]
    fn exits_in_groups_remove_the_hint() {
        let menu = menu_of(&[friends(), work()]);

        assert!(
            menu.items.is_empty(),
            "several networks keep exits in groups"
        );
        assert_eq!(menu.empty_hint(), None);
    }

    #[test]
    fn empty_groups_keep_the_hint() {
        let plain = empty_net("main", "friends");
        let other = empty_net("ab12cd", "work");

        let menu = menu_of(&[plain, other]);

        assert_eq!(menu.empty_hint(), Some(EMPTY_HINT));
    }
}

mod net_rows {
    use super::*;

    #[test]
    fn submenu_has_invite_serve_participants_and_members() {
        // given
        let net = nets_of(&[friends()]).remove(0);

        // when
        let rows = net_rows(&net, None);

        // then
        assert_eq!(row(&rows, "invite:main").text, "Invite a member");
        let serve = row(&rows, "serve:main");
        assert_eq!(
            serve.text,
            "Let members of \"friends\" open sites via this computer"
        );
        assert_eq!(serve.kind, RowKind::Check);
        assert_eq!(
            row(&rows, "members:main").text,
            "Open Members of \"friends\"…"
        );
    }

    #[test]
    fn serve_check_follows_the_node() {
        let state = StateSpec {
            serves_exit: true,
            ..Default::default()
        };
        let serving = NetSpec {
            state: Some(state_json(&state)),
            ..friends()
        };
        let net = nets_of(&[serving]).remove(0);

        assert!(row(&net_rows(&net, None), "serve:main").checked);
    }

    #[test]
    fn participants_are_rows_and_offline_ones_are_grey() {
        let net = nets_of(&[friends()]).remove(0);

        let rows = net_rows(&net, None);

        let peers: Vec<(&str, bool)> = rows
            .iter()
            .filter(|r| r.id == "peer:main")
            .map(|r| (r.text.as_str(), r.secondary))
            .collect();
        let expected = [
            ("peer-one · RU — online", false),
            ("peer-two — online", false),
            ("peer-three — offline", true),
        ];
        assert_eq!(peers, expected);
    }

    #[test]
    fn copied_invite_note_replaces_the_item_text() {
        let net = nets_of(&[friends()]).remove(0);

        let rows = net_rows(&net, Some("Invite to \"friends\" copied ✓"));

        assert_eq!(
            row(&rows, "invite:main").text,
            "Invite to \"friends\" copied ✓"
        );
    }

    #[test]
    fn ten_participants_end_with_a_count_of_the_rest() {
        let net = nets_of(&[friends_with_peers(10)]).remove(0);

        let rows = net_rows(&net, None);

        let peers: Vec<&RowSpec> = rows.iter().filter(|r| r.id == "peer:main").collect();
        assert_eq!(peers.len(), 9, "eight participants and the count row");
        assert_eq!(peers[8].text, "and 2 more — open Members…");
    }

    #[test]
    fn eight_participants_have_no_count_row() {
        let net = nets_of(&[friends_with_peers(8)]).remove(0);

        let rows = net_rows(&net, None);

        let peers: Vec<&RowSpec> = rows.iter().filter(|r| r.id == "peer:main").collect();
        assert_eq!(peers.len(), 8);
        assert!(peers.iter().all(|r| !r.text.starts_with("and ")));
    }

    #[test]
    fn joining_network_has_only_inactive_rows() {
        let joining = NetSpec {
            joining: true,
            ..NetSpec::keyless("ab12cd")
        };
        let net = nets_of(&[friends(), joining]).remove(1);

        let rows = net_rows(&net, None);

        assert!(!rows.is_empty());
        assert!(rows.iter().all(|r| !r.enabled), "{rows:?}");
    }
}

mod layout_key {
    use super::*;

    #[test]
    fn renamed_participant_keeps_the_key() {
        let before = menu_of(&[friends(), work()]);
        let mut after = before.clone();
        after.nets[0].peers[0].name = "renamed".to_string();

        assert_eq!(layout_key(&before), layout_key(&after));
    }

    #[test]
    fn added_participant_changes_the_key() {
        let before = menu_of(&[friends(), work()]);
        let mut after = before.clone();
        let extra = after.nets[0].peers[0].clone();
        after.nets[0].peers.push(extra);

        assert_ne!(layout_key(&before), layout_key(&after));
    }

    #[test]
    fn vanished_network_changes_the_key() {
        let before = menu_of(&[friends(), work()]);
        let mut after = before.clone();
        after.nets.pop();

        assert_ne!(layout_key(&before), layout_key(&after));
    }
}

mod invite_in_managed_network {
    use super::*;
    use serde_json::json;

    #[test]
    fn member_gets_a_grey_row_instead_of_the_invite() {
        // given
        let extra = json!({ "me_admin": false, "can_invite": false });
        let net = managed_net(extra);

        // when
        let rows = net_rows(&net, None);

        // then
        let invite = row(&rows, "invite:main");
        assert_eq!(invite.text, "Only admins can invite to \"friends\"");
        assert!(!invite.enabled, "a member cannot click it");
        assert_eq!(invite.kind, RowKind::Item);
    }

    #[test]
    fn admin_keeps_the_usual_invite() {
        let extra = json!({ "me_admin": true, "can_invite": true });
        let net = managed_net(extra);

        let rows = net_rows(&net, None);

        let invite = row(&rows, "invite:main");
        assert_eq!(invite.text, "Invite a member");
        assert!(invite.enabled);
    }

    #[test]
    fn a_network_that_allows_inviting_shows_the_usual_invite() {
        let extra = json!({ "me_admin": false, "can_invite": true });
        let net = managed_net(extra);

        let rows = net_rows(&net, None);

        assert_eq!(row(&rows, "invite:main").text, "Invite a member");
        assert!(row(&rows, "invite:main").enabled);
    }

    #[test]
    fn grey_row_keeps_the_place_and_the_layout_key() {
        // given
        let extra = json!({ "me_admin": false, "can_invite": false });
        let managed = managed_net(extra);
        let peer = nets_of(&[friends()]).remove(0);

        // when
        let ids = |net: &NetMenu| -> Vec<String> {
            net_rows(net, None).into_iter().map(|r| r.id).collect()
        };

        // then
        assert_eq!(ids(&managed), ids(&peer), "same ids in the same order");
    }

    #[test]
    fn other_rows_of_a_managed_member_stay_usable() {
        let extra = json!({ "me_admin": false, "can_invite": false });
        let net = managed_net(extra);

        let rows = net_rows(&net, None);

        assert!(row(&rows, "members:main").enabled);
        assert!(row(&rows, "serve:main").enabled);
    }
}

/// The only network of a summary whose entry has `extra` fields merged in.
fn managed_net(extra: serde_json::Value) -> NetMenu {
    let mut json = overview_json(&[friends()]);
    for (key, value) in extra.as_object().expect("an object") {
        json["nets"][0][key] = value.clone();
    }
    let vis = visible_slots(parse_overview(&json).expect("a summary"));
    nets_from_slots(&vis).remove(0)
}

fn nets_of(specs: &[NetSpec]) -> Vec<NetMenu> {
    nets_from_slots(&visible(specs))
}

fn visible(specs: &[NetSpec]) -> Vec<crate::gui::tray::app::overview::SlotView> {
    visible_slots(parse_overview(&overview_json(specs)).expect("a summary"))
}

/// The menu for several networks: participants in `nets`, exits in `groups`.
fn menu_of(specs: &[NetSpec]) -> ExitMenu {
    let vis = visible(specs);
    let mut menu = disabled_exit_menu(ProxyState::Off(ProxyOff::DisabledInSettings));
    menu.enabled = true;
    menu.nets = nets_from_slots(&vis);
    menu.groups = groups_from_slots(&vis);
    menu
}

fn row<'a>(rows: &'a [RowSpec], id: &str) -> &'a RowSpec {
    rows.iter()
        .find(|r| r.id == id)
        .unwrap_or_else(|| panic!("no row {id}: {rows:?}"))
}
