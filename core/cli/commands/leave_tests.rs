use serde_json::json;

use super::*;
use crate::cli::commands::client::Target;

fn node(base: &str, explicit_net: bool) -> Node {
    let nets = json!({"nets": [
        {"id": "main", "label": "home"},
        {"id": "k3j2h4", "label": "work"},
    ]});
    let target = Target {
        path: "c.json".into(),
        dir: ".".into(),
        port: 1,
        config_given: false,
    };
    Node {
        target,
        base: base.into(),
        nets,
        explicit_net,
    }
}

#[test]
fn label_is_the_chosen_network() {
    assert_eq!(net_label(&node("/api/net/k3j2h4", true)), "work");
}

#[test]
fn label_without_net_is_the_first_network() {
    assert_eq!(net_label(&node("/api", false)), "home");
}

#[test]
fn label_of_nothing_is_a_plain_word() {
    assert_eq!(label_text(None), "the network");
}
