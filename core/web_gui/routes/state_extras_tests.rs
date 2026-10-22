//! The set of keys that `apply_state_extras` adds to the serialized `NodeSnapshot::default()`
//! in the `/api/state` response: a literal in the test, no external files. The main snapshot key set
//! is fixed in `web/snapshot_tests.rs`, `node.invite_join` in `state_tests.rs`.

use serde_json::{json, Value};

use super::{apply_state_extras, StateExtras};
use crate::web_gui::NodeSnapshot;

/// Top-level key paths and those nested in `node`, sorted.
fn paths(value: &Value) -> Vec<String> {
    let mut out: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
    let node = value["node"].as_object().unwrap();
    out.extend(node.keys().map(|k| format!("node.{k}")));
    out.sort();
    out
}

fn extras() -> StateExtras {
    StateExtras {
        port: 7070,
        config_rev: 3,
        identity: "persistent",
        identity_reason: None,
        network: Value::Null,
        invite_join: None,
    }
}

#[test]
fn extras_add_exactly_the_listed_paths_to_default_snapshot() {
    // given: the default serialized snapshot
    let mut value = serde_json::to_value(NodeSnapshot::default()).unwrap();
    let before = paths(&value);
    // when
    apply_state_extras(&mut value, extras());
    // then: exactly these paths were added and nothing else
    let added: Vec<String> = paths(&value)
        .into_iter()
        .filter(|p| !before.contains(p))
        .collect();
    let want = [
        "config_rev",
        "network",
        "node.identity",
        "node.identity_reason",
        "node.invite_join",
        "node.version",
        "node.web_addr",
    ];
    assert_eq!(added, want);
    assert_eq!(
        paths(&value).len(),
        before.len() + want.len(),
        "something that was added was already in the snapshot"
    );
}

#[test]
fn extras_values_come_from_the_arguments() {
    // given
    let mut value = serde_json::to_value(NodeSnapshot::default()).unwrap();
    // when
    apply_state_extras(&mut value, extras());
    // then
    assert_eq!(value["config_rev"], json!(3));
    assert_eq!(value["node"]["web_addr"], json!("127.0.0.1:7070"));
    assert_eq!(value["node"]["identity"], json!("persistent"));
    assert_eq!(value["node"]["identity_reason"], Value::Null);
}
