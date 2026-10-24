//! Tests of the order of the menu items that come and go with the node's state: the notes right after "Open Mesh Town…"
//! and the "Add a network…" item after the network block.
// OPEN: `menu_order` is a pure description of the order that `menu_build`, `vote_item` and `nets_ui` follow when they
// insert widgets; the tests need `top_ids` and `middle_ids` with the signatures used below.

use super::*;
use crate::gui::tray::app::exit_menu::{disabled_exit_menu, ExitMenu};
use crate::gui::tray::app::local_api::stub_node::{friends, overview_json, work, NetSpec};
use crate::gui::tray::app::nets_menu::{groups_from_slots, nets_from_slots};
use crate::gui::tray::app::overview::{parse_overview, visible_slots};
use crate::gui::tray::app::proxy_state::{ProxyOff, ProxyState};

mod top_ids {
    use super::*;

    #[test]
    fn without_notes_the_status_and_open_come_first() {
        assert_eq!(top_ids(&[], false), ["status", "open"]);
    }

    #[test]
    fn vote_comes_right_after_open() {
        assert_eq!(top_ids(&[], true), ["status", "open", "vote"]);
    }

    #[test]
    fn deleted_network_note_is_between_open_and_vote() {
        assert_eq!(top_ids(&[4], true), ["status", "open", "notice:4", "vote"]);
    }

    #[test]
    fn note_alone_follows_open() {
        assert_eq!(top_ids(&[4], false), ["status", "open", "notice:4"]);
    }

    #[test]
    fn several_notes_keep_their_order() {
        assert_eq!(
            top_ids(&[4, 9], true),
            ["status", "open", "notice:4", "notice:9", "vote"]
        );
    }
}

mod middle_ids {
    use super::*;

    #[test]
    fn without_a_network_there_is_no_add_item() {
        let menu = flat_menu();

        assert_eq!(middle_ids(&menu, false), ["invite", "serve"]);
    }

    #[test]
    fn flat_menu_of_one_network_has_add_after_serve() {
        let menu = flat_menu();

        assert_eq!(middle_ids(&menu, true), ["invite", "serve", "add"]);
    }

    #[test]
    fn nested_menu_has_add_after_the_network_submenus() {
        let menu = nested_menu(&[friends(), work()]);

        assert_eq!(
            middle_ids(&menu, true),
            ["net-menu:main", "net-menu:ab12cd", "add"]
        );
    }

    #[test]
    fn nested_menu_does_not_list_the_flat_items() {
        let menu = nested_menu(&[friends(), work()]);

        let ids = middle_ids(&menu, true);

        assert!(
            !ids.iter().any(|id| id == "invite" || id == "serve"),
            "{ids:?}"
        );
    }
}

fn flat_menu() -> ExitMenu {
    disabled_exit_menu(ProxyState::Off(ProxyOff::DisabledInSettings))
}

fn nested_menu(specs: &[NetSpec]) -> ExitMenu {
    let vis = visible_slots(parse_overview(&overview_json(specs)).expect("a summary"));
    let mut menu = flat_menu();
    menu.enabled = true;
    menu.nets = nets_from_slots(&vis);
    menu.groups = groups_from_slots(&vis);
    menu
}
