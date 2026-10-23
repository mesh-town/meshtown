//! Tests of the summary parser: which shape is a summary, which networks are visible, which one is the main.

use super::*;
use crate::gui::tray::app::local_api::stub_node::{
    friends, overview_json, state_json, work, NetSpec, StateSpec,
};
use serde_json::json;

mod parse_overview {
    use super::*;

    #[test]
    fn two_networks_give_two_slots_in_order() {
        // given
        let json = overview_json(&[friends(), work()]);

        // when
        let slots = parse_overview(&json).expect("a summary");

        // then
        let ids: Vec<&str> = slots.iter().map(|s| s.slot.as_str()).collect();
        assert_eq!(ids, ["main", "ab12cd"]);
    }

    #[test]
    fn slot_carries_summary_fields() {
        // given
        let net = NetSpec {
            admitting: 1,
            voting: 2,
            ..friends()
        };

        // when
        let slots = slots_of(&[net]);

        // then
        let s = &slots[0];
        assert_eq!(s.label.as_deref(), Some("friends"));
        assert!(s.proxy && !s.joining);
        assert_eq!(s.status, "online");
        assert_eq!((s.peers_online, s.peers_total), (2, 3));
        assert_eq!((s.members_voting, s.members_admitting), (2, 1));
    }

    #[test]
    fn slot_carries_its_state_object() {
        // given / when
        let slots = slots_of(&[friends(), work()]);

        // then
        assert_eq!(slots[1].state["peers"][0]["name"], "peer-b");
    }

    #[test]
    fn plain_state_reply_is_not_a_summary() {
        let state = state_json(&StateSpec::default());

        assert!(parse_overview(&state).is_none());
    }

    #[test]
    fn nets_that_is_not_an_array_is_not_a_summary() {
        let json = json!({ "api": 2, "nets": "main", "slots": [] });

        assert!(parse_overview(&json).is_none());
    }

    #[test]
    fn empty_arrays_give_no_slots() {
        let json = json!({ "api": 2, "nets": [], "slots": [] });

        assert_eq!(parse_overview(&json), Some(Vec::new()));
    }

    #[test]
    fn missing_fields_become_zero_none_false() {
        // given
        let json = json!({
            "nets": [{ "id": "main" }],
            "slots": [{ "slot": "main", "state": null }],
        });

        // when
        let slots = parse_overview(&json).expect("a summary");

        // then
        let s = &slots[0];
        assert_eq!(s.slot, "main");
        assert_eq!(s.label, None);
        assert!(!s.joining && !s.proxy);
        assert_eq!((s.peers_online, s.peers_total), (0, 0));
        assert_eq!((s.members_voting, s.members_admitting), (0, 0));
    }

    #[test]
    fn fields_of_foreign_types_become_defaults() {
        // given
        let json = json!({
            "nets": [{ "id": "main", "label": 5, "proxy": "yes", "peers_online": "many", "joining": 1 }],
            "slots": [{ "slot": "main", "state": null }],
        });

        // when
        let slots = parse_overview(&json).expect("a summary");

        // then
        let s = &slots[0];
        assert_eq!(s.label, None);
        assert!(!s.proxy && !s.joining);
        assert_eq!(s.peers_online, 0);
    }
}

mod visible_slots {
    use super::*;

    #[test]
    fn network_without_key_is_hidden() {
        let all = slots_of(&[friends(), NetSpec::keyless("ab12cd")]);

        assert_eq!(ids(visible_slots(all)), ["main"]);
    }

    #[test]
    fn joining_network_without_key_is_visible() {
        let joining = NetSpec {
            joining: true,
            ..NetSpec::keyless("ab12cd")
        };
        let all = slots_of(&[friends(), joining]);

        assert_eq!(ids(visible_slots(all)), ["main", "ab12cd"]);
    }

    #[test]
    fn broken_slot_is_hidden() {
        let all = slots_of(&[friends(), NetSpec::broken("zz")]);

        assert_eq!(ids(visible_slots(all)), ["main"]);
    }

    #[test]
    fn order_follows_the_summary() {
        let all = slots_of(&[work(), friends()]);

        assert_eq!(ids(visible_slots(all)), ["ab12cd", "main"]);
    }
}

mod main_slot {
    use super::*;

    #[test]
    fn slot_that_holds_the_proxy_wins() {
        let proxy_here = NetSpec {
            proxy: true,
            ..work()
        };
        let vis = slots_of(&[
            NetSpec {
                proxy: false,
                ..friends()
            },
            proxy_here,
        ]);

        assert_eq!(main_slot(&vis).map(|s| s.slot.as_str()), Some("ab12cd"));
    }

    #[test]
    fn without_a_proxy_slot_the_first_is_main() {
        let vis = slots_of(&[
            NetSpec {
                proxy: false,
                ..friends()
            },
            work(),
        ]);

        assert_eq!(main_slot(&vis).map(|s| s.slot.as_str()), Some("main"));
    }

    #[test]
    fn no_slots_no_main() {
        assert!(main_slot(&[]).is_none());
    }
}

fn slots_of(specs: &[NetSpec]) -> Vec<SlotView> {
    parse_overview(&overview_json(specs)).expect("a summary")
}

fn ids(slots: Vec<SlotView>) -> Vec<String> {
    slots.into_iter().map(|s| s.slot).collect()
}
