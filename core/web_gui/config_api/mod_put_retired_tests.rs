//! `PUT /api/config` with a retired field in the body: the field is accepted, logged and dropped.

use super::*;
use crate::node::output::{log_page, log_summary};

const WARNING: &str = "deprecated fields ignored and dropped on the next write: transport.mode";

#[test]
fn put_with_retired_mode_is_ok_logged_and_not_written() {
    // given: a fresh page state and a patch with the retired `transport.mode` next to a real change
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let rev = state.file_report.rev;
    let seq_before = log_summary(0).last_seq;
    let body = put_body(
        rev,
        serde_json::json!({"node": {"name": "alpha"}, "transport": {"mode": "relay_only"}}),
    );
    // when: PUT
    let (status, v) = body_json(state.put(&body, &StubNode::new()));
    let text = std::fs::read_to_string(dir.join("config.json")).expect("config file");
    cleanup(&dir);
    // then: 200, only the name changed, the file has no `mode`, and the log has the warning
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["changed"], serde_json::json!(["node.name"]), "{v}");
    assert!(
        !text.contains("relay_only"),
        "the retired value is in the file: {text}"
    );
    assert!(
        !text.contains("\"transport\""),
        "the retired section is in the file: {text}"
    );
    let lines: Vec<String> = log_page(seq_before, 4000)
        .lines
        .into_iter()
        .filter(|l| l.tag == "config")
        .map(|l| l.text)
        .collect();
    assert!(
        lines.iter().any(|l| l == WARNING),
        "no warning in {lines:?}"
    );
}

#[test]
fn get_has_no_transport_section() {
    // given: a fresh page state
    let dir = temp_dir();
    let state = new_state(&dir);
    // when: the page document is built
    let doc = state.page_config_document();
    cleanup(&dir);
    // then: there is no `transport` in it
    assert!(doc.get("transport").is_none(), "{doc}");
}
