//! Tests of `exit_menu`: the tray submenu "exit choice" — building the node list from a snapshot.

use super::*;
use crate::app::proxy_state::{ProxyOff, ProxyState};
use serde_json::json;

/// A shim with the old name: the tests supply JSON, the code reads the selection.
fn exit_menu_from_json(value: &serde_json::Value) -> ExitMenu {
    exit_menu_from_snapshot(&crate::app::snapshot::TraySnapshot::from_value(value))
}

mod exit_menu_from_json {
    use super::*;

    mod when_snapshot_has_no_route_field {
        use super::*;

        #[test]
        fn menu_is_enabled_with_no_items() {
            // given: the node is online, an old snapshot format — there is no "route" key at all
            let state = json!({
                "node": { "status": "online", "id_full": "me00" },
                "peers": []
            });

            // when
            let menu = exit_menu_from_json(&state);

            // then
            assert!(menu.enabled, "the node is online — the submenu is available");
            assert!(
                menu.items.is_empty(),
                "no route.exits and no other peers with exit"
            );
        }
    }

    mod when_foreign_exit_peer_and_selected_offline_exit_both_present {
        use super::*;

        fn fixture() -> serde_json::Value {
            json!({
                "node": { "status": "online", "id_full": "me00" },
                "peers": [
                    {
                        "id": "aa11", "id_full": "aa11full", "name": "Daisy",
                        "self": false, "online": true, "exit": true
                    },
                    {
                        "id": "bb22", "id_full": "bb22full", "name": "Sedge",
                        "self": false, "online": false, "exit": true
                    }
                ],
                "route": {
                    "state": "unavailable",
                    "exits": [
                        {
                            "id": "bb22", "id_full": "bb22full", "name": "Sedge",
                            "online": false, "exit": true, "active": false
                        }
                    ]
                }
            })
        }

        #[test]
        fn both_are_listed() {
            let menu = exit_menu_from_json(&fixture());
            assert_eq!(
                menu.items.len(),
                2,
                "both a peer advertising itself as an exit and a chosen one (even if offline) are in the list: {:?}",
                menu.items
            );
        }

        #[test]
        fn unselected_foreign_exit_is_unchecked_without_suffix() {
            let menu = exit_menu_from_json(&fixture());
            let item = menu
                .items
                .iter()
                .find(|i| i.id_full == "aa11full")
                .expect("aa11full is another exit node, must be in the list");
            assert!(!item.checked, "aa11full is not chosen in route.exits");
            assert_eq!(item.label, "Daisy");
        }

        #[test]
        fn selected_offline_exit_is_checked_with_offline_suffix() {
            let menu = exit_menu_from_json(&fixture());
            let item = menu
                .items
                .iter()
                .find(|i| i.id_full == "bb22full")
                .expect("bb22full is the chosen exit, must be in the list");
            assert!(item.checked, "bb22full is in route.exits");
            assert_eq!(item.label, "Sedge (offline)");
        }
    }

    mod when_own_id_appears_among_exits {
        use super::*;

        #[test]
        fn own_node_is_excluded() {
            // given: our own id is both in peers (self:true, exit:true) and in route.exits (the config
            // accepts our own id, the choice skips it; the submenu is also without our own node)
            let state = json!({
                "node": { "status": "online", "id_full": "me00" },
                "peers": [
                    {
                        "id": "me", "id_full": "me00", "name": "Me",
                        "self": true, "online": true, "exit": true
                    }
                ],
                "route": {
                    "state": "direct",
                    "exits": [
                        {
                            "id": "me", "id_full": "me00", "name": "Me",
                            "online": true, "exit": true, "active": false
                        }
                    ]
                }
            });

            let menu = exit_menu_from_json(&state);

            assert!(
                menu.items.is_empty(),
                "the own node must not get into the submenu: {:?}",
                menu.items
            );
        }
    }

    mod when_snapshot_is_garbage {
        use super::*;

        #[test]
        fn menu_is_disabled() {
            let menu = exit_menu_from_json(&json!("not a node snapshot"));
            assert!(!menu.enabled, "garbage instead of a snapshot — the submenu is unavailable");
            assert!(menu.items.is_empty());
        }
    }
}

mod when_name_is_missing {
    use super::*;

    #[test]
    fn label_falls_back_to_short_id() {
        // given: another exit node without a name (today's reality for other peers)
        let state = json!({
            "node": { "status": "online", "id_full": "me00" },
            "peers": [
                {
                    "id": "aa11", "id_full": "aa11full", "name": null,
                    "self": false, "online": true, "exit": true
                }
            ]
        });

        let menu = exit_menu_from_json(&state);

        let item = menu
            .items
            .iter()
            .find(|i| i.id_full == "aa11full")
            .expect("aa11full must be in the list");
        assert_eq!(
            item.label, "Member aa11",
            "no name (including a member known only from the address cache) — the label is \"Member <id>\", not an empty string"
        );
    }
}

mod disabled_exit_menu {
    use super::*;

    #[test]
    fn returns_disabled_empty_menu() {
        let menu = disabled_exit_menu(ProxyState::Off(ProxyOff::DisabledInSettings));
        assert!(!menu.enabled);
        assert!(menu.items.is_empty());
        assert_eq!(
            menu.route,
            RouteLine::Direct,
            "grey view — we don't show the route"
        );
        assert_eq!(menu.proxy, ProxyState::Off(ProxyOff::DisabledInSettings));
    }

    #[test]
    fn carries_the_given_proxy_address() {
        let menu = disabled_exit_menu(ProxyState::On("127.0.0.1:1080".to_string()));
        assert!(!menu.enabled);
        assert_eq!(menu.proxy, ProxyState::On("127.0.0.1:1080".to_string()));
    }
}
