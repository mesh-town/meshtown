use serde_json::json;

use super::*;

#[test]
fn a_key_has_kind_label_and_id() {
    let reply = json!({"label": "ourgame", "id_hex": "0123456789abcdef"});
    assert_eq!(
        lines(&reply),
        ["kind: key", "label: ourgame", "id: 0123456789abcdef"]
    );
}

#[test]
fn an_invite_adds_place_and_slot() {
    let reply = json!({"label": null, "id_hex": "ab", "place": "new", "slot": "k3j2"});
    assert_eq!(
        lines(&reply),
        [
            "kind: invite",
            "label: -",
            "id: ab",
            "place: new",
            "slot: k3j2"
        ]
    );
}
