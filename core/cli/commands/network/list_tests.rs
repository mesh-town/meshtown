use serde_json::{json, Value};

use super::*;

fn net(extra: Value) -> Value {
    let mut base = json!({
        "id": "main", "label": "ourgame", "id_hex": "0123456789abcdef", "status": "online",
        "joining": false, "me_excluded": false, "me_pending": false, "me_admin": false,
        "peers_online": 2, "peers_total": 5,
    });
    for (k, v) in extra.as_object().unwrap() {
        base[k] = v.clone();
    }
    base
}

fn one_row(extra: Value) -> Vec<String> {
    let rows = rows(&json!({"nets": [net(extra)]})).unwrap();
    rows.into_iter().next().unwrap()
}

#[test]
fn main_with_label_and_admin_role() {
    let r = one_row(json!({"me_admin": true}));
    assert_eq!(r, ["main", "ourgame", "01234567", "online", "admin", "2/5"]);
}

#[test]
fn empty_slot_has_dashes() {
    let r = one_row(json!({"label": null, "id_hex": null, "status": "waiting_key"}));
    assert_eq!(r, ["main", "-", "-", "waiting_key", "-", "2/5"]);
}

#[test]
fn broken_slot_shows_no_role() {
    let r = one_row(json!({"status": "broken", "me_admin": true, "label": null}));
    assert_eq!((r[3].as_str(), r[4].as_str()), ("broken", "-"));
}

#[test]
fn joining_pending_excluded_override_status() {
    let status = |extra| one_row(extra)[3].clone();
    assert_eq!(status(json!({"joining": true})), "joining");
    assert_eq!(
        status(json!({"joining": true, "me_pending": true})),
        "pending"
    );
    let all = json!({"joining": true, "me_pending": true, "me_excluded": true});
    assert_eq!(status(all), "excluded");
}

#[test]
fn empty_nets_gives_no_rows() {
    assert!(rows(&json!({"nets": []})).unwrap().is_empty());
}

#[test]
fn no_nets_array_is_an_error() {
    assert!(rows(&json!({"api": 1})).is_err());
}
