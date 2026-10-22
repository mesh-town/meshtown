//! Tests of `exit_menu`: the route line, the proxy and the "serves as an exit" flag.

use super::*;
use crate::app::proxy_state::{ProxyOff, ProxyState};
use serde_json::json;

/// A shim with the old name: the tests supply JSON, the code reads the selection.
fn exit_menu_from_json(value: &serde_json::Value) -> ExitMenu {
    exit_menu_from_snapshot(&crate::app::snapshot::TraySnapshot::from_value(value))
}

/// `ExitMenu.route`: `route.state`/`route.exits[]` of the snapshot →
/// `RouteLine` for the tray status line.
mod route {
    use super::*;

    #[test]
    fn no_route_field_is_direct() {
        let state = json!({ "node": { "status": "online", "id_full": "me00" } });
        assert_eq!(exit_menu_from_json(&state).route, RouteLine::Direct);
    }

    #[test]
    fn exit_active_with_name_is_via_named() {
        let state = json!({
            "node": { "status": "online", "id_full": "me00" },
            "route": {
                "state": "exit",
                "exits": [
                    {
                        "id": "aa11", "id_full": "aa11full", "name": "Sedge",
                        "online": true, "exit": true, "active": true
                    }
                ]
            }
        });
        assert_eq!(
            exit_menu_from_json(&state).route,
            RouteLine::Via("Sedge".to_string())
        );
    }

    #[test]
    fn exit_active_without_name_is_via_member_id() {
        let state = json!({
            "node": { "status": "online", "id_full": "me00" },
            "route": {
                "state": "exit",
                "exits": [
                    {
                        "id": "aa11", "id_full": "aa11full", "name": null,
                        "online": true, "exit": true, "active": true
                    }
                ]
            }
        });
        assert_eq!(
            exit_menu_from_json(&state).route,
            RouteLine::Via("Member aa11".to_string())
        );
    }

    #[test]
    fn unavailable_with_one_selected_names_it() {
        let state = json!({
            "node": { "status": "online", "id_full": "me00" },
            "route": {
                "state": "unavailable",
                "exits": [
                    {
                        "id": "bb22", "id_full": "bb22full", "name": "Daisy",
                        "online": false, "exit": true, "active": false
                    }
                ]
            }
        });
        assert_eq!(
            exit_menu_from_json(&state).route,
            RouteLine::Unavailable(Some("Daisy".to_string()))
        );
    }

    #[test]
    fn unavailable_with_several_selected_is_anonymous() {
        let state = json!({
            "node": { "status": "online", "id_full": "me00" },
            "route": {
                "state": "unavailable",
                "exits": [
                    {
                        "id": "bb22", "id_full": "bb22full", "name": "Daisy",
                        "online": false, "exit": true, "active": false
                    },
                    {
                        "id": "cc33", "id_full": "cc33full", "name": "Sedge",
                        "online": false, "exit": true, "active": false
                    }
                ]
            }
        });
        assert_eq!(
            exit_menu_from_json(&state).route,
            RouteLine::Unavailable(None)
        );
    }

    #[test]
    fn two_selected_exits_both_checked_no_panic() {
        // two chosen exits from an old config: the menu doesn't collapse them (the first click will fix it)
        let state = json!({
            "node": { "status": "online", "id_full": "me00" },
            "route": {
                "state": "unavailable",
                "exits": [
                    {
                        "id": "bb22", "id_full": "bb22full", "name": "Daisy",
                        "online": false, "exit": true, "active": false
                    },
                    {
                        "id": "cc33", "id_full": "cc33full", "name": "Sedge",
                        "online": false, "exit": true, "active": false
                    }
                ]
            }
        });
        let menu = exit_menu_from_json(&state);
        let checked: Vec<&str> = menu
            .items
            .iter()
            .filter(|i| i.checked)
            .map(|i| i.id_full.as_str())
            .collect();
        assert_eq!(checked, vec!["bb22full", "cc33full"]);
        assert_eq!(menu.route, RouteLine::Unavailable(None));
    }

    #[test]
    fn unavailable_with_no_exits_is_anonymous() {
        let state = json!({
            "node": { "status": "online", "id_full": "me00" },
            "route": {
                "state": "unavailable",
                "exits": []
            }
        });
        assert_eq!(
            exit_menu_from_json(&state).route,
            RouteLine::Unavailable(None)
        );
    }

    #[test]
    fn garbage_input_is_direct() {
        assert_eq!(
            exit_menu_from_json(&json!("not a node snapshot")).route,
            RouteLine::Direct
        );
    }
}

/// `ExitMenu.proxy`: the SOCKS5 address, only if enabled (`ProxyState`).
mod proxy {
    use super::*;

    #[test]
    fn socks_enabled_exposes_listen_address() {
        let state = json!({
            "node": { "status": "online", "id_full": "me00" },
            "socks": { "enabled": true, "listen": "127.0.0.1:1080" }
        });
        assert_eq!(
            exit_menu_from_json(&state).proxy,
            ProxyState::On("127.0.0.1:1080".to_string())
        );
    }

    #[test]
    fn socks_disabled_is_off() {
        let state = json!({
            "node": { "status": "online", "id_full": "me00" },
            "socks": { "enabled": false, "listen": "127.0.0.1:1080" }
        });
        assert_eq!(exit_menu_from_json(&state).proxy,
            ProxyState::Off(ProxyOff::DisabledInSettings)
        );
    }

    #[test]
    fn no_socks_field_is_off() {
        let state = json!({ "node": { "status": "online", "id_full": "me00" } });
        assert_eq!(exit_menu_from_json(&state).proxy,
            ProxyState::Off(ProxyOff::DisabledInSettings)
        );
    }
}

mod serves_exit {
    use super::*;

    #[test]
    fn serves_exit_from_snapshot() {
        let on = json!({"node": {"id_full": "me00"}, "exit": {"enabled": true}});
        let off = json!({"node": {"id_full": "me00"}, "exit": {"enabled": false}});
        let missing = json!({"node": {"id_full": "me00"}});

        assert!(exit_menu_from_json(&on).serves_exit);
        assert!(!exit_menu_from_json(&off).serves_exit);
        assert!(!exit_menu_from_json(&missing).serves_exit);
    }

    #[test]
    fn menu_off_does_not_serve() {
        assert!(!disabled_exit_menu(ProxyState::Off(ProxyOff::DisabledInSettings)).serves_exit);
    }
}
