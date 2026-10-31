use serde_json::json;

use super::*;
use crate::cli::commands::client::Target;

fn node(base: &str) -> Node {
    let target = Target {
        path: "c.json".into(),
        dir: ".".into(),
        port: 1,
        config_given: false,
    };
    Node {
        target,
        base: base.into(),
        nets: json!({"nets": []}),
        explicit_net: false,
    }
}

#[test]
fn path_without_note_is_plain() {
    assert_eq!(invite_path(&node("/api"), None), "/api/network/invite");
}

#[test]
fn path_escapes_the_note_and_follows_the_slot() {
    assert_eq!(
        invite_path(&node("/api/net/k3j2h4"), Some("Ann & Bo")),
        "/api/net/k3j2h4/network/invite?for=Ann%20%26%20Bo"
    );
}

#[test]
fn reply_without_invite_is_an_error() {
    assert!(print_invite(&json!({})).is_err());
}
