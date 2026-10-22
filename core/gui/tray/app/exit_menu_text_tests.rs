//! Tests of `ExitItem::menu_text`: the red sign next to a chosen offline exit.

use super::*;
use crate::gui::tray::app::proxy_state::{ProxyOff, ProxyState};
use crate::gui::tray::app::tray_texts::{direct_text, SUBMENU_TITLE};
use serde_json::json;

/// A shim with the old name: the tests supply JSON, the code reads the selection.
fn exit_menu_from_json(value: &serde_json::Value) -> ExitMenu {
    exit_menu_from_snapshot(&crate::gui::tray::app::snapshot::TraySnapshot::from_value(
        value,
    ))
}

fn snapshot(peers: serde_json::Value, route: serde_json::Value) -> serde_json::Value {
    json!({
        "node": { "status": "online", "id_full": "me00" },
        "peers": peers,
        "route": route,
    })
}

mod menu_text {
    use super::*;

    mod when_foreign_exit_peer_and_selected_offline_exit_both_present {
        use super::*;

        fn fixture() -> serde_json::Value {
            snapshot(
                json!([
                    {
                        "id": "aa11", "id_full": "aa11full", "name": "Daisy",
                        "self": false, "online": true, "exit": true
                    },
                    {
                        "id": "bb22", "id_full": "bb22full", "name": "Sedge",
                        "self": false, "online": false, "exit": true
                    }
                ]),
                json!({
                    "state": "unavailable",
                    "exits": [
                        {
                            "id": "bb22", "id_full": "bb22full", "name": "Sedge",
                            "online": false, "exit": true, "active": false
                        }
                    ]
                }),
            )
        }

        #[test]
        fn selected_offline_exit_menu_text_has_red_mark() {
            let menu = exit_menu_from_json(&fixture());
            let item = menu
                .items
                .iter()
                .find(|i| i.id_full == "bb22full")
                .expect("bb22full is the chosen exit, must be in the list");
            assert!(!item.online, "bb22full is offline");
            assert_eq!(item.menu_text(), "🔴 Sedge (offline)");
        }

        #[test]
        fn unselected_online_exit_menu_text_is_plain_label() {
            let menu = exit_menu_from_json(&fixture());
            let item = menu
                .items
                .iter()
                .find(|i| i.id_full == "aa11full")
                .expect("aa11full is another exit node, must be in the list");
            assert!(item.online, "aa11full is online");
            assert_eq!(item.menu_text(), "Daisy");
        }
    }

    mod when_offline_exit_peer_is_not_selected {
        use super::*;

        #[test]
        fn offline_peer_is_listed_unchecked_without_red_mark() {
            let state = snapshot(
                json!([
                    {
                        "id": "bb22", "id_full": "bb22full", "name": "Sedge",
                        "self": false, "online": false, "exit": true
                    }
                ]),
                json!({ "state": "direct", "exits": [] }),
            );

            let menu = exit_menu_from_json(&state);
            let item = menu
                .items
                .iter()
                .find(|i| i.id_full == "bb22full")
                .expect("bb22full advertises itself as an exit, must be in the list");
            assert!(!item.checked, "bb22full is not chosen");
            assert!(!item.online, "bb22full is offline");
            assert_eq!(item.menu_text(), "Sedge (offline)");
        }
    }

    mod when_selected_exit_is_online {
        use super::*;

        #[test]
        fn selected_online_exit_has_no_red_mark() {
            let state = snapshot(
                json!([
                    {
                        "id": "bb22", "id_full": "bb22full", "name": "Sedge",
                        "self": false, "online": true, "exit": true
                    }
                ]),
                json!({
                    "state": "exit",
                    "exits": [
                        {
                            "id": "bb22", "id_full": "bb22full", "name": "Sedge",
                            "online": true, "exit": true, "active": true
                        }
                    ]
                }),
            );

            let menu = exit_menu_from_json(&state);
            let item = menu
                .items
                .iter()
                .find(|i| i.id_full == "bb22full")
                .expect("bb22full is the chosen exit, must be in the list");
            assert!(item.checked, "bb22full is in route.exits");
            assert_eq!(item.menu_text(), "Sedge");
        }
    }
}

mod submenu_texts {
    use super::*;

    #[test]
    fn title_and_direct_item_use_site_wording() {
        assert_eq!(SUBMENU_TITLE, "Exit via…");
        assert_eq!(direct_text(), "Directly, without an exit");
    }
}

mod empty_hint {
    use super::*;

    fn peer(exit: bool) -> serde_json::Value {
        json!([{
            "id": "aa11", "id_full": "aa11full", "name": "Daisy",
            "self": false, "online": true, "exit": exit
        }])
    }

    #[test]
    fn no_items_gives_hint() {
        // given: in the network, there are no members
        let state = snapshot(json!([]), json!({ "state": "direct", "exits": [] }));

        // when
        let hint = exit_menu_from_json(&state).empty_hint();

        // then
        assert_eq!(hint, Some(EMPTY_HINT));
        assert_eq!(
            EMPTY_HINT,
            "No member lets sites open via their computer yet"
        );
    }

    #[test]
    fn advertising_peer_hides_hint() {
        let state = snapshot(peer(true), json!({ "state": "direct", "exits": [] }));
        assert_eq!(exit_menu_from_json(&state).empty_hint(), None);
    }

    #[test]
    fn chosen_offline_exit_hides_hint() {
        // given: nobody serves, but one is chosen and offline — its item is in the list
        let state = snapshot(
            peer(false),
            json!({
                "state": "unavailable",
                "exits": [{
                    "id": "bb22", "id_full": "bb22full", "name": "Sedge",
                    "online": false, "exit": false, "active": false
                }]
            }),
        );
        let menu = exit_menu_from_json(&state);
        assert!(!menu.items.is_empty(), "the chosen exit must be an item");
        assert_eq!(menu.empty_hint(), None);
    }

    #[test]
    fn off_menu_gives_hint() {
        assert_eq!(
            disabled_exit_menu(ProxyState::Off(ProxyOff::DisabledInSettings)).empty_hint(),
            Some(EMPTY_HINT)
        );
    }
}

mod first_line_carries_country {
    use super::*;

    #[test]
    fn active_exit_route_has_country() {
        let state = snapshot(
            json!([]),
            json!({
                "state": "exit",
                "exits": [{
                    "id": "aa11", "id_full": "aa11full", "name": "peer-one", "country": "DE",
                    "online": true, "exit": true, "active": true
                }]
            }),
        );
        assert_eq!(
            exit_menu_from_json(&state).route,
            RouteLine::Via("peer-one · DE".to_string())
        );
    }

    #[test]
    fn single_unavailable_route_has_country_without_suffix() {
        let state = snapshot(
            json!([]),
            json!({
                "state": "unavailable",
                "exits": [{
                    "id": "aa11", "id_full": "aa11full", "name": "peer-one", "country": "DE",
                    "online": false, "exit": true, "active": false
                }]
            }),
        );
        assert_eq!(
            exit_menu_from_json(&state).route,
            RouteLine::Unavailable(Some("peer-one · DE".to_string()))
        );
    }
}
