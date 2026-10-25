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

mod managed_fields {
    use super::*;

    #[test]
    fn summary_entry_carries_kind_admin_and_invite_right() {
        // given
        let json =
            summary_with(json!({ "kind": "Managed", "me_admin": true, "can_invite": false }));

        // when
        let s = first_slot(&json);

        // then
        assert!(s.managed, "kind Managed");
        assert!(s.admin, "me_admin");
        assert!(!s.can_invite, "can_invite false is kept");
    }

    #[test]
    fn no_fields_mean_a_peer_network_where_inviting_is_allowed() {
        let s = first_slot(&summary_with(json!({})));

        assert!(!s.managed && !s.admin);
        assert!(s.can_invite, "an old node never forbids inviting");
    }

    #[test]
    fn peer_kind_is_not_managed() {
        let json = summary_with(json!({ "kind": "Peer", "me_admin": false, "can_invite": true }));

        let s = first_slot(&json);

        assert!(!s.managed && !s.admin && s.can_invite);
    }

    #[test]
    fn unknown_kind_reads_as_peer() {
        let s = first_slot(&summary_with(json!({ "kind": "Council" })));

        assert!(!s.managed);
    }

    #[test]
    fn state_network_is_the_fallback_source() {
        // given
        let mut json = summary_with(json!({}));
        json["slots"][0]["state"]["network"] =
            json!({ "kind": "Managed", "me_role": "admin", "can_invite": false });

        // when
        let s = first_slot(&json);

        // then
        assert!(s.managed && s.admin);
        assert!(!s.can_invite);
    }

    #[test]
    fn member_role_in_the_state_is_not_admin() {
        let mut json = summary_with(json!({}));
        json["slots"][0]["state"]["network"] =
            json!({ "kind": "Managed", "me_role": "member", "can_invite": false });

        let s = first_slot(&json);

        assert!(s.managed && !s.admin);
    }

    #[test]
    fn summary_entry_wins_over_the_state() {
        // given
        let mut json =
            summary_with(json!({ "kind": "Managed", "me_admin": true, "can_invite": true }));
        json["slots"][0]["state"]["network"] =
            json!({ "kind": "Peer", "me_role": "member", "can_invite": false });

        // when
        let s = first_slot(&json);

        // then
        assert!(s.managed && s.admin && s.can_invite);
    }

    #[test]
    fn fields_of_foreign_types_become_peer_defaults() {
        let json = summary_with(json!({ "kind": 5, "me_admin": "yes", "can_invite": "no" }));

        let s = first_slot(&json);

        assert!(!s.managed && !s.admin);
        assert!(s.can_invite, "an unreadable flag does not forbid inviting");
    }
}

mod notices {
    use super::*;

    #[test]
    fn deleted_network_notes_come_in_the_order_of_the_array() {
        // given
        let mut json = overview_json(&[friends()]);
        json["notices"] = json!([
            { "id": 4, "kind": "net_deleted", "label": "work", "id_hex": "ab", "by": "x", "by_name": "Ann", "at_unix": 1 },
            { "id": 9, "kind": "net_deleted", "label": "home", "id_hex": "cd", "by": "y", "by_name": "Bob", "at_unix": 2 },
        ]);

        // when
        let notes = parse_notices(&json);

        // then
        let pairs: Vec<(u64, &str)> = notes.iter().map(|n| (n.id, n.label.as_str())).collect();
        assert_eq!(pairs, [(4, "work"), (9, "home")]);
    }

    #[test]
    fn no_notices_field_means_no_notes() {
        assert!(parse_notices(&overview_json(&[friends()])).is_empty());
    }

    #[test]
    fn notices_that_is_not_an_array_means_no_notes() {
        let mut json = overview_json(&[friends()]);
        json["notices"] = json!("work");

        assert!(parse_notices(&json).is_empty());
    }

    #[test]
    fn broken_entries_are_skipped() {
        // given
        let mut json = overview_json(&[friends()]);
        json["notices"] = json!([
            { "kind": "net_deleted", "label": "no id" },
            { "id": "7", "kind": "net_deleted", "label": "id is text" },
            { "id": 5, "kind": "something_else", "label": "other kind" },
            { "id": 6, "kind": "net_deleted", "label": "fine" },
        ]);

        // when
        let notes = parse_notices(&json);

        // then
        let ids: Vec<u64> = notes.iter().map(|n| n.id).collect();
        assert_eq!(ids, [6]);
    }

    #[test]
    fn a_note_with_a_null_label_is_kept_with_an_empty_label() {
        let mut json = overview_json(&[friends()]);
        json["notices"] = json!([{ "id": 3, "kind": "net_deleted", "label": null }]);

        let notes = parse_notices(&json);

        assert_eq!(
            notes,
            [Notice {
                id: 3,
                label: String::new()
            }]
        );
        assert_eq!(
            crate::gui::tray::app::nets_texts::net_deleted_text(&notes[0].label),
            "A network was deleted by an admin — open Members…"
        );
    }

    #[test]
    fn summary_slots_do_not_depend_on_notices() {
        let mut json = overview_json(&[friends(), work()]);
        json["notices"] = json!([{ "id": 1, "kind": "net_deleted", "label": "gone" }]);

        assert_eq!(
            ids(parse_overview(&json).expect("a summary")),
            ["main", "ab12cd"]
        );
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
    fn network_with_a_key_and_no_label_is_visible() {
        let unnamed = NetSpec {
            label: None,
            id_hex: Some("bb"),
            ..NetSpec::keyless("ab12cd")
        };
        let all = slots_of(&[friends(), unnamed]);

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

/// A one-network summary whose first entry has `extra` fields merged in (e.g. `kind`, `me_admin`).
fn summary_with(extra: serde_json::Value) -> serde_json::Value {
    let mut json = overview_json(&[friends()]);
    for (key, value) in extra.as_object().expect("an object") {
        json["nets"][0][key] = value.clone();
    }
    json
}

fn first_slot(json: &serde_json::Value) -> SlotView {
    parse_overview(json).expect("a summary").remove(0)
}
