//! Tests for parsing the `keep_ports` body field of `POST /api/network/join|create`.

use serde_json::{json, Map, Value};

use super::wants_keep_ports;

fn map(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

#[test]
fn only_json_true_keeps_ports() {
    for body in [
        json!({}),
        json!({"keep_ports": false}),
        json!({"keep_ports": "yes"}),
        json!({"keep_ports": 1}),
        json!({"keep_ports": null}),
    ] {
        assert!(!wants_keep_ports(&map(body.clone())), "{body}");
    }
    assert!(wants_keep_ports(&map(json!({"keep_ports": true}))));
}
