//! Tests for `PUT /api/config` for the `ports` section: live application and 422 refusal. The harness is
//! from `mod_tests.rs` via `use super::*;`.

use super::*;

#[test]
fn put_ports_published_applies_live() {
    // given: a PUT with a valid `ports.published`.
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let node = StubNode::new();
    let rev = state.file_report.rev;
    let body = put_body(
        rev,
        serde_json::json!({"ports": {"published": [{"port": 8080, "name": "web", "for": "all"}]}}),
    );

    // when: state.put / then 200, the path in `changed`, no restart needed.
    let (status, v) = body_json(state.put(&body, &node));
    assert_eq!(status, StatusCode::OK);
    assert!(v["changed"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p == "ports.published"));
    assert!(!state.pending_restart_paths().contains(&"ports.published"));
    assert_eq!(state.file_settings.net.ports_published.len(), 1);
    let applied = node.applied.lock().unwrap();
    assert_eq!(applied.len(), 1);
    assert_eq!(applied[0], vec!["ports.published".to_string()]);
    cleanup(&dir);
}

#[test]
fn put_ports_duplicate_port_is_invalid() {
    // given: a PUT with a repeated port.
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let node = StubNode::new();
    let rev = state.file_report.rev;
    let body = put_body(
        rev,
        serde_json::json!({"ports": {"published": [
            {"port": 80, "name": "a", "for": "all"},
            {"port": 80, "name": "b", "for": "all"}
        ]}}),
    );

    // then: 422 `invalid`, in `fields` the path `ports.published` and the `expected()` text of the row.
    let (status, v) = body_json(state.put(&body, &node));
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(v["error"], "invalid");
    let fields = v["fields"].as_array().unwrap();
    assert!(fields.iter().any(|f| {
        f["path"] == "ports.published"
            && f["message"] == "a list of publications {port, name, for} without repeated ports, up to 64, for is \"all\" or a list of ids"
    }));
    assert!(node.applied.lock().unwrap().is_empty());
    cleanup(&dir);
}
