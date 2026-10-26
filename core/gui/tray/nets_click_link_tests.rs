//! Tests of the page links of menu clicks and of the facts taken from the menu model.

use super::*;
use crate::gui::tray::app::exit_menu::disabled_exit_menu;
use crate::gui::tray::app::local_api::stub_node::{friends, work, NetSpec};
use crate::gui::tray::app::proxy_state::{ProxyOff, ProxyState};

mod link_for {
    use super::*;

    const BASE: &str = "http://127.0.0.1:8080/?token=T";

    #[test]
    fn open_is_the_base_without_an_anchor() {
        assert_eq!(
            link_for(&MenuTarget::Open, BASE, &MenuFacts::default()),
            Some(BASE.to_string())
        );
    }

    #[test]
    fn members_and_participant_rows_point_at_the_network() {
        let target = parse_menu_id("peer:ab12cd");

        let link = link_for(&target, BASE, &MenuFacts::default());

        assert_eq!(
            link.as_deref(),
            Some("http://127.0.0.1:8080/?token=T#peers/ab12cd")
        );
        assert_eq!(
            link,
            link_for(
                &parse_menu_id("members:ab12cd"),
                BASE,
                &MenuFacts::default()
            )
        );
    }

    #[test]
    fn admit_with_two_networks_points_at_the_network_with_the_request() {
        let mut facts = facts(&[("main", false, false), ("ab12cd", false, false)]);
        facts.admit_slot = Some("ab12cd".into());

        let link = link_for(&MenuTarget::Admit, BASE, &facts);

        assert_eq!(
            link.as_deref(),
            Some("http://127.0.0.1:8080/?token=T#peers/ab12cd")
        );
    }

    #[test]
    fn admit_with_one_network_points_at_the_member_list() {
        let link = link_for(&MenuTarget::Admit, BASE, &MenuFacts::default());

        assert_eq!(
            link.as_deref(),
            Some("http://127.0.0.1:8080/?token=T#peers")
        );
    }

    #[test]
    fn admit_with_two_networks_and_no_slot_points_at_the_member_list() {
        let facts = facts(&[("main", false, false), ("ab12cd", false, false)]);

        let link = link_for(&MenuTarget::Admit, BASE, &facts);

        assert_eq!(
            link.as_deref(),
            Some("http://127.0.0.1:8080/?token=T#peers")
        );
    }

    #[test]
    fn commands_have_no_link() {
        assert_eq!(
            link_for(&MenuTarget::Quit, BASE, &MenuFacts::default()),
            None
        );
        assert_eq!(
            link_for(&MenuTarget::Directly, BASE, &MenuFacts::default()),
            None
        );
    }
}

mod menu_facts {
    use super::*;

    #[test]
    fn nested_menu_lists_every_network() {
        // given
        let menu = menu_of(&[friends(), work()]);

        // when
        let f = menu_facts(&menu);

        // then
        let slots: Vec<&str> = f.nets.iter().map(|n| n.slot.as_str()).collect();
        assert_eq!(slots, ["main", "ab12cd"]);
        assert_eq!(f.nets[0].label, "friends");
    }

    #[test]
    fn chosen_exit_marks_only_its_network() {
        let f = menu_facts(&menu_of(&[friends(), work()]));

        assert!(!f.nets[0].exit_chosen, "nothing is chosen in friends");
        assert!(f.nets[1].exit_chosen, "peer-b is chosen in work");
    }

    #[test]
    fn flat_menu_has_no_network_facts() {
        let menu = disabled_exit_menu(ProxyState::Off(ProxyOff::DisabledInSettings));

        assert!(menu_facts(&menu).nets.is_empty());
    }

    #[test]
    fn admit_slot_is_the_first_network_with_a_request() {
        let first = NetSpec {
            admitting: 1,
            ..friends()
        };
        let second = NetSpec {
            admitting: 1,
            ..work()
        };

        let f = menu_facts(&menu_of(&[first, second]));

        assert_eq!(f.admit_slot.as_deref(), Some("main"));
    }

    #[test]
    fn a_waiting_node_picks_the_network() {
        let second = NetSpec {
            admitting: 1,
            ..work()
        };

        let f = menu_facts(&menu_of(&[friends(), second]));

        assert_eq!(f.admit_slot.as_deref(), Some("ab12cd"));
    }

    #[test]
    fn quiet_networks_have_no_admit_slot() {
        assert_eq!(menu_facts(&menu_of(&[friends(), work()])).admit_slot, None);
    }
}

mod added_items {
    use super::*;

    const BASE: &str = "http://127.0.0.1:8080/?token=T";

    #[test]
    fn add_a_network_opens_the_add_form() {
        let link = link_for(&MenuTarget::Add, BASE, &MenuFacts::default());

        assert_eq!(
            link.as_deref(),
            Some("http://127.0.0.1:8080/?token=T#peers/add")
        );
    }

    #[test]
    fn add_a_network_ignores_an_anchor_already_in_the_base() {
        let link = link_for(
            &MenuTarget::Add,
            "http://127.0.0.1:8080/#peers",
            &MenuFacts::default(),
        );

        assert_eq!(link.as_deref(), Some("http://127.0.0.1:8080/#peers/add"));
    }

    #[test]
    fn deleted_network_note_opens_the_member_list() {
        let link = link_for(&MenuTarget::Notice(7), BASE, &MenuFacts::default());

        assert_eq!(
            link.as_deref(),
            Some("http://127.0.0.1:8080/?token=T#peers")
        );
    }

    #[test]
    fn deleted_network_note_does_not_depend_on_the_admit_network() {
        let mut with_request = facts(&[("main", false, false), ("ab12cd", false, false)]);
        with_request.admit_slot = Some("ab12cd".into());

        let link = link_for(&MenuTarget::Notice(7), BASE, &with_request);

        assert_eq!(
            link.as_deref(),
            Some("http://127.0.0.1:8080/?token=T#peers")
        );
    }
}
