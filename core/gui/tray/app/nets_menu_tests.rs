//! Tests of the per-network menu data: submenu titles, participant rows and the exit groups.

use super::*;
use crate::gui::tray::app::exit_menu::ExitItem;
use crate::gui::tray::app::local_api::stub_node::{
    empty_net, friends, friends_with_peers, overview_json, state_json, work, NetSpec, StateSpec,
};
use crate::gui::tray::app::overview::{parse_overview, visible_slots, SlotView};

mod nets_from_slots {
    use super::*;

    #[test]
    fn one_menu_per_visible_network() {
        // given
        let vis = visible(&[friends(), work(), NetSpec::keyless("cd34ef")]);

        // when
        let nets = nets_from_slots(&vis);

        // then
        let slots: Vec<&str> = nets.iter().map(|n| n.slot.as_str()).collect();
        assert_eq!(slots, ["main", "ab12cd"]);
        assert_eq!(nets[0].label, "friends");
    }

    #[test]
    fn title_counts_online_participants() {
        let net = NetSpec {
            peers_online: 2,
            peers_total: 3,
            ..friends()
        };

        assert_eq!(title_of(net), "friends — 2 of 3 online");
    }

    #[test]
    fn title_has_no_confirm_part_without_waiting_nodes() {
        let net = NetSpec {
            peers_online: 3,
            peers_total: 3,
            admitting: 0,
            ..friends()
        };

        assert_eq!(title_of(net), "friends — 3 of 3 online");
    }

    #[test]
    fn title_counts_nodes_waiting_for_confirmation() {
        let net = NetSpec {
            peers_online: 2,
            peers_total: 3,
            admitting: 1,
            ..friends()
        };

        assert_eq!(title_of(net), "friends — 2 of 3 online, 1 to confirm");
    }

    #[test]
    fn title_of_offline_network_says_no_connection() {
        let net = NetSpec {
            label: Some("my devices"),
            status: "offline",
            ..friends()
        };

        let nets = nets_from_slots(&visible(&[net]));

        assert_eq!(nets[0].title, "my devices — no connection");
        assert!(!nets[0].online, "an offline network is not online");
    }

    #[test]
    fn joining_network_is_marked_and_titled() {
        let joining = NetSpec {
            joining: true,
            ..NetSpec::keyless("ab12cd")
        };

        let nets = nets_from_slots(&visible(&[friends(), joining]));

        assert!(nets[1].joining);
        assert_eq!(nets[1].title, "New network — joining…");
    }

    #[test]
    fn participants_come_from_the_state_of_the_network() {
        // given / when
        let nets = nets_from_slots(&visible(&[friends(), work()]));

        // then
        let rows = &nets[0].peers;
        assert_eq!(names(rows), ["peer-one", "peer-two", "peer-three"]);
        assert!(rows[0].exit && rows[0].online);
        assert_eq!(rows[0].country.as_deref(), Some("RU"));
        assert!(!rows[2].online, "peer-three is offline");
        assert_eq!(names(&nets[1].peers), ["peer-b"]);
    }

    #[test]
    fn eight_participants_all_get_rows() {
        let nets = nets_from_slots(&visible(&[friends_with_peers(8)]));

        assert_eq!(nets[0].peers.len(), 8);
        assert_eq!(nets[0].peers_more, 0);
    }

    #[test]
    fn ten_participants_show_eight_and_count_the_rest() {
        let nets = nets_from_slots(&visible(&[friends_with_peers(10)]));

        assert_eq!(nets[0].peers.len(), 8);
        assert_eq!(nets[0].peers_more, 2);
    }

    #[test]
    fn serving_flag_comes_from_the_state() {
        let state = StateSpec {
            serves_exit: true,
            ..Default::default()
        };
        let serving = NetSpec {
            state: Some(state_json(&state)),
            ..friends()
        };

        let nets = nets_from_slots(&visible(&[serving, work()]));

        assert!(nets[0].serves_exit);
        assert!(!nets[1].serves_exit);
    }

    #[test]
    fn vote_and_waiting_counts_come_from_the_summary() {
        let net = NetSpec {
            voting: 1,
            admitting: 2,
            ..friends()
        };

        let nets = nets_from_slots(&visible(&[net]));

        assert_eq!((nets[0].voting, nets[0].admitting), (1, 2));
    }

    #[test]
    fn network_without_state_has_only_the_summary() {
        let net = NetSpec {
            state: None,
            ..friends()
        };

        let nets = nets_from_slots(&visible(&[net]));

        assert_eq!(nets[0].title, "friends — 2 of 3 online");
        assert!(nets[0].peers.is_empty() && !nets[0].serves_exit);
    }
}

mod groups_from_slots {
    use super::*;

    #[test]
    fn one_group_per_visible_network() {
        let groups = groups_from_slots(&visible(&[friends(), work(), NetSpec::keyless("cd34ef")]));

        let titles: Vec<&str> = groups.iter().map(|g| g.title.as_str()).collect();
        assert_eq!(titles, ["friends", "work"]);
        assert_eq!(groups[1].slot, "ab12cd");
    }

    #[test]
    fn group_lists_only_peers_that_offer_an_exit() {
        let groups = groups_from_slots(&visible(&[friends()]));

        let labels: Vec<&str> = groups[0].items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(labels, ["peer-one · RU"]);
    }

    #[test]
    fn chosen_exit_is_checked() {
        let groups = groups_from_slots(&visible(&[friends(), work()]));

        let expected = ExitItem {
            id_full: "peerb".to_string(),
            label: "peer-b".to_string(),
            checked: true,
            online: true,
        };
        assert_eq!(groups[1].items, [expected]);
        assert!(!groups[0].items[0].checked, "nothing is chosen in friends");
    }

    #[test]
    fn network_without_exits_keeps_an_empty_group() {
        let plain = empty_net("main", "friends");

        let groups = groups_from_slots(&visible(&[plain, work()]));

        assert_eq!(groups.len(), 2);
        assert!(groups[0].items.is_empty());
    }
}

fn visible(specs: &[NetSpec]) -> Vec<SlotView> {
    visible_slots(parse_overview(&overview_json(specs)).expect("a summary"))
}

fn title_of(net: NetSpec) -> String {
    nets_from_slots(&visible(&[net])).remove(0).title
}

fn names(rows: &[PeerRow]) -> Vec<&str> {
    rows.iter().map(|r| r.name.as_str()).collect()
}
