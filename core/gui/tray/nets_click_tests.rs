//! Tests of the menu click logic: item ids, page links and node commands for flat and nested menus.

use super::*;
use crate::gui::tray::app::exit_menu::{disabled_exit_menu, ExitMenu};
use crate::gui::tray::app::local_api::stub_node::{overview_json, NetSpec};
use crate::gui::tray::app::nets_menu::{groups_from_slots, nets_from_slots};
use crate::gui::tray::app::overview::{parse_overview, visible_slots};
use crate::gui::tray::app::proxy_state::{ProxyOff, ProxyState};
use crate::gui::tray::NodeCmd;

mod parse_menu_id {
    use super::*;

    #[test]
    fn fixed_items_keep_their_ids() {
        assert_eq!(parse_menu_id("open"), MenuTarget::Open);
        assert_eq!(parse_menu_id("vote"), MenuTarget::Vote);
        assert_eq!(parse_menu_id("node"), MenuTarget::Node);
        assert_eq!(parse_menu_id("quit"), MenuTarget::Quit);
    }

    #[test]
    fn flat_invite_and_serve_have_no_slot() {
        assert_eq!(parse_menu_id("invite"), MenuTarget::Invite(None));
        assert_eq!(parse_menu_id("serve"), MenuTarget::Serve(None));
    }

    #[test]
    fn invite_and_serve_of_a_network_carry_its_slot() {
        assert_eq!(
            parse_menu_id("invite:ab12cd"),
            MenuTarget::Invite(Some("ab12cd".into()))
        );
        assert_eq!(
            parse_menu_id("serve:ab12cd"),
            MenuTarget::Serve(Some("ab12cd".into()))
        );
    }

    #[test]
    fn empty_exit_id_is_directly() {
        assert_eq!(parse_menu_id("exit:"), MenuTarget::Directly);
    }

    #[test]
    fn exit_without_second_colon_is_a_flat_exit() {
        let expected = MenuTarget::Exit {
            slot: None,
            id: "abc".into(),
        };

        assert_eq!(parse_menu_id("exit:abc"), expected);
    }

    #[test]
    fn exit_with_slot_splits_at_the_first_colon() {
        let expected = MenuTarget::Exit {
            slot: Some("ab12cd".into()),
            id: "abc".into(),
        };

        assert_eq!(parse_menu_id("exit:ab12cd:abc"), expected);
    }

    #[test]
    fn participant_and_members_rows_open_members_of_the_slot() {
        let expected = MenuTarget::Members("ab12cd".into());

        assert_eq!(parse_menu_id("peer:ab12cd"), expected);
        assert_eq!(parse_menu_id("members:ab12cd"), expected);
    }

    #[test]
    fn ids_that_are_not_clicks_are_unknown() {
        for id in ["", "zzz", "status", "proxy", "net:ab12cd", "exit:ab12cd:"] {
            assert_eq!(parse_menu_id(id), MenuTarget::Unknown, "id {id:?}");
        }
    }
}

mod node_cmd_for {
    use super::*;

    #[test]
    fn directly_with_two_networks_clears_only_those_with_a_chosen_exit() {
        // given
        let facts = facts(&[("main", false, false), ("ab12cd", false, true)]);

        // when
        let cmds = node_cmd_for(&MenuTarget::Directly, None, false, &facts);

        // then
        assert_eq!(cmds, [NodeCmd::ClearExits(Some("ab12cd".into()))]);
    }

    #[test]
    fn directly_with_two_chosen_exits_clears_both() {
        let facts = facts(&[("main", false, true), ("ab12cd", false, true)]);

        let cmds = node_cmd_for(&MenuTarget::Directly, None, false, &facts);

        let expected = [
            NodeCmd::ClearExits(Some("main".into())),
            NodeCmd::ClearExits(Some("ab12cd".into())),
        ];
        assert_eq!(cmds, expected);
    }

    #[test]
    fn directly_in_a_flat_menu_clears_the_flat_slot() {
        let cmds = node_cmd_for(
            &MenuTarget::Directly,
            Some("ab12cd"),
            false,
            &MenuFacts::default(),
        );

        assert_eq!(cmds, [NodeCmd::ClearExits(Some("ab12cd".into()))]);
    }

    #[test]
    fn flat_exit_goes_to_the_flat_slot() {
        let target = MenuTarget::Exit {
            slot: None,
            id: "abc".into(),
        };

        let cmds = node_cmd_for(&target, Some("ab12cd"), false, &MenuFacts::default());

        assert_eq!(
            cmds,
            [NodeCmd::ToggleExit {
                slot: Some("ab12cd".into()),
                id: "abc".into()
            }]
        );
    }

    #[test]
    fn flat_exit_of_the_main_network_has_no_slot() {
        let target = MenuTarget::Exit {
            slot: None,
            id: "abc".into(),
        };

        let cmds = node_cmd_for(&target, None, false, &MenuFacts::default());

        assert_eq!(
            cmds,
            [NodeCmd::ToggleExit {
                slot: None,
                id: "abc".into()
            }]
        );
    }

    #[test]
    fn exit_of_a_network_keeps_its_slot() {
        let target = MenuTarget::Exit {
            slot: Some("ab12cd".into()),
            id: "abc".into(),
        };
        let facts = facts(&[("main", false, false), ("ab12cd", false, false)]);

        let cmds = node_cmd_for(&target, None, false, &facts);

        assert_eq!(
            cmds,
            [NodeCmd::ToggleExit {
                slot: Some("ab12cd".into()),
                id: "abc".into()
            }]
        );
    }

    #[test]
    fn flat_serve_flips_the_flat_flag() {
        let off = node_cmd_for(
            &MenuTarget::Serve(None),
            Some("ab12cd"),
            false,
            &MenuFacts::default(),
        );
        let on = node_cmd_for(
            &MenuTarget::Serve(None),
            Some("ab12cd"),
            true,
            &MenuFacts::default(),
        );

        assert_eq!(
            off,
            [NodeCmd::SetExitServer {
                slot: Some("ab12cd".into()),
                on: true
            }]
        );
        assert_eq!(
            on,
            [NodeCmd::SetExitServer {
                slot: Some("ab12cd".into()),
                on: false
            }]
        );
    }

    #[test]
    fn serve_of_a_network_flips_the_flag_of_that_network() {
        let facts = facts(&[("main", false, false), ("ab12cd", true, false)]);

        let cmds = node_cmd_for(
            &MenuTarget::Serve(Some("ab12cd".into())),
            None,
            false,
            &facts,
        );

        assert_eq!(
            cmds,
            [NodeCmd::SetExitServer {
                slot: Some("ab12cd".into()),
                on: false
            }]
        );
    }

    #[test]
    fn invite_goes_to_the_clicked_network_or_the_flat_slot() {
        let flat = node_cmd_for(
            &MenuTarget::Invite(None),
            Some("ab12cd"),
            false,
            &MenuFacts::default(),
        );
        let nested = node_cmd_for(
            &MenuTarget::Invite(Some("cd34ef".into())),
            None,
            false,
            &MenuFacts::default(),
        );

        assert_eq!(flat, [NodeCmd::CopyInvite(Some("ab12cd".into()))]);
        assert_eq!(nested, [NodeCmd::CopyInvite(Some("cd34ef".into()))]);
    }

    #[test]
    fn targets_the_core_handles_itself_give_no_command() {
        let targets = [
            MenuTarget::Open,
            MenuTarget::Vote,
            MenuTarget::Node,
            MenuTarget::Quit,
            MenuTarget::Members("ab12cd".into()),
            MenuTarget::Unknown,
        ];
        for t in targets {
            assert!(
                node_cmd_for(&t, Some("ab12cd"), false, &MenuFacts::default()).is_empty(),
                "{t:?}"
            );
        }
    }
}

/// Facts for networks given as `(slot, serves_exit, exit_chosen)`.
fn facts(nets: &[(&str, bool, bool)]) -> MenuFacts {
    let net = |&(slot, serves_exit, exit_chosen): &(&str, bool, bool)| NetFact {
        slot: slot.to_string(),
        label: slot.to_string(),
        serves_exit,
        exit_chosen,
    };
    MenuFacts {
        nets: nets.iter().map(net).collect(),
        vote_slot: None,
    }
}

/// The menu of several networks: participants in `nets`, exits in `groups`.
fn menu_of(specs: &[NetSpec]) -> ExitMenu {
    let vis = visible_slots(parse_overview(&overview_json(specs)).expect("a summary"));
    let mut menu = disabled_exit_menu(ProxyState::Off(ProxyOff::DisabledInSettings));
    menu.nets = nets_from_slots(&vis);
    menu.groups = groups_from_slots(&vis);
    menu
}

#[path = "nets_click_link_tests.rs"]
mod link_tests;
