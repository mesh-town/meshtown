//! Tests of `exit_menu`: the country code in the submenu item label.

use super::*;
use serde_json::json;

/// A shim with the old name: the tests supply JSON, the code reads the selection.
fn exit_menu_from_json(value: &serde_json::Value) -> ExitMenu {
    exit_menu_from_snapshot(&crate::gui::tray::app::snapshot::TraySnapshot::from_value(
        value,
    ))
}

fn peer_state(country: serde_json::Value, online: bool) -> serde_json::Value {
    json!({
        "node": { "status": "online", "id_full": "me00" },
        "peers": [
            {
                "id": "aa11", "id_full": "aa11full", "name": "Kitchen",
                "self": false, "online": online, "exit": true, "country": country
            }
        ]
    })
}

fn peer_label(state: &serde_json::Value) -> String {
    let menu = exit_menu_from_json(state);
    menu.items
        .iter()
        .find(|i| i.id_full == "aa11full")
        .expect("aa11full must be in the list")
        .label
        .clone()
}

#[test]
fn online_peer_with_country_shows_code() {
    let state = peer_state(json!("RU"), true);
    assert_eq!(peer_label(&state), "Kitchen · RU");
}

#[test]
fn offline_peer_with_country_shows_code_then_suffix() {
    let state = peer_state(json!("RU"), false);
    assert_eq!(peer_label(&state), "Kitchen · RU (offline)");
}

#[test]
fn no_country_field_is_old_node_label_unchanged() {
    // given: an old node sent a snapshot without the "country" field at all — no panic, no "· null" is written
    let state = json!({
        "node": { "status": "online", "id_full": "me00" },
        "peers": [
            {
                "id": "aa11", "id_full": "aa11full", "name": "Kitchen",
                "self": false, "online": true, "exit": true
            }
        ]
    });
    assert_eq!(peer_label(&state), "Kitchen");
}

#[test]
fn country_not_a_string_is_ignored() {
    let state = peer_state(json!(5), true);
    assert_eq!(peer_label(&state), "Kitchen");
}

#[test]
fn unnamed_peer_with_country() {
    let state = json!({
        "node": { "status": "online", "id_full": "me00" },
        "peers": [
            {
                "id": "aa11", "id_full": "aa11full", "name": null,
                "self": false, "online": true, "exit": true, "country": "RU"
            }
        ]
    });
    assert_eq!(peer_label(&state), "Member aa11 · RU");
}

#[test]
fn chosen_exit_with_country_shows_code() {
    // given: a chosen exit from route.exits[] with its own country
    let state = json!({
        "node": { "status": "online", "id_full": "me00" },
        "peers": [],
        "route": {
            "state": "unavailable",
            "exits": [
                {
                    "id": "bb22", "id_full": "bb22full", "name": "Sedge",
                    "online": true, "exit": true, "active": false, "country": "DE"
                }
            ]
        }
    });
    let menu = exit_menu_from_json(&state);
    let item = menu
        .items
        .iter()
        .find(|i| i.id_full == "bb22full")
        .expect("bb22full is the chosen exit, must be in the list");
    assert_eq!(item.label, "Sedge · DE");
}
