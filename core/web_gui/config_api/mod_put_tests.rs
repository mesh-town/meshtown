//! Tests for `config_api::ConfigState::put` — moved out of `mod_tests.rs` (400-line limit per
//! file); the harness (`temp_dir`, `new_state`, `put_body`, `body_json`, `StubNode`) stays in
//! the parent and comes here through `use super::*;`.

use super::*;
use crate::transport::net::NodeSecret;

#[test]
fn put_socks_port_out_of_range_is_invalid() {
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let node = StubNode::new();
    let rev = state.file_report.rev;
    let body = put_body(rev, serde_json::json!({"socks": {"port": 80}}));
    let resp = state.put(&body, &node);
    let (status, v) = body_json(resp);
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(v["error"], "invalid");
    let fields = v["fields"].as_array().unwrap();
    assert!(fields
        .iter()
        .any(|f| f["path"] == "socks.port" && f["message"] == "a port 1024…65535"));
    cleanup(&dir);
}

#[test]
fn put_autostart_enabled_is_not_available() {
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let node = StubNode::new();
    let rev = state.file_report.rev;
    let body = put_body(rev, serde_json::json!({"autostart": {"enabled": true}}));
    let resp = state.put(&body, &node);
    let (status, v) = body_json(resp);
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(v["error"], "not_available");
    // The text does not name the number of a nonexistent module —
    // one common wording "coming in a later version" for both refusal reasons.
    assert_eq!(
        v["message"], "coming in a later version",
        "the refusal text must not mention a service module, {v:?}"
    );
    cleanup(&dir);
}

#[test]
fn put_full_tunnel_enabled_is_not_available() {
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let node = StubNode::new();
    let rev = state.file_report.rev;
    let body = put_body(rev, serde_json::json!({"full_tunnel": {"enabled": true}}));
    let resp = state.put(&body, &node);
    let (status, v) = body_json(resp);
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(v["error"], "not_available");
    // The same common text as for autostart — does not mention TUN or a module number.
    assert_eq!(
        v["message"], "coming in a later version",
        "the refusal text must not mention the TUN module, {v:?}"
    );
    cleanup(&dir);
}

#[test]
fn put_read_only_web_port_is_ignored() {
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let node = StubNode::new();
    let rev = state.file_report.rev;
    let original_port = state.file_settings.process.web_port;
    let body = put_body(rev, serde_json::json!({"web": {"port": 9999}}));
    let resp = state.put(&body, &node);
    let (status, v) = body_json(resp);
    assert_eq!(status, StatusCode::OK);
    assert_eq!(state.file_settings.process.web_port, original_port);
    assert_eq!(v["changed"].as_array().unwrap().len(), 0);
    cleanup(&dir);
}

#[test]
fn put_partial_config_only_changes_named_field() {
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let node = StubNode::new();
    let rev = state.file_report.rev;
    let before = state.file_settings.clone();
    let body = put_body(rev, serde_json::json!({"socks": {"port": 1090}}));
    let resp = state.put(&body, &node);
    let (status, _v) = body_json(resp);
    assert_eq!(status, StatusCode::OK);
    let mut expected = before;
    expected.process.socks_port = 1090;
    assert_eq!(state.file_settings, expected);
    cleanup(&dir);
}

#[test]
fn put_immediate_field_applies_without_restart() {
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let node = StubNode::new();
    let rev = state.file_report.rev;
    let body = put_body(rev, serde_json::json!({"checks": {"interval_sec": 600}}));
    let resp = state.put(&body, &node);
    let (status, _v) = body_json(resp);
    assert_eq!(status, StatusCode::OK);
    let applied = node.applied.lock().unwrap();
    assert_eq!(applied.len(), 1);
    assert_eq!(applied[0], vec!["checks.interval_sec".to_string()]);
    cleanup(&dir);
}

#[test]
fn put_restart_field_does_not_apply_immediately() {
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let node = StubNode::new();
    let rev = state.file_report.rev;
    let body = put_body(rev, serde_json::json!({"socks": {"port": 1090}}));
    let resp = state.put(&body, &node);
    let (status, _v) = body_json(resp);
    assert_eq!(status, StatusCode::OK);
    assert!(node.applied.lock().unwrap().is_empty());
    assert_eq!(state.pending_restart_paths(), vec!["socks.port"]);
    cleanup(&dir);
}

#[test]
fn put_route_exits_applies_without_restart() {
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let node = StubNode::new();
    let rev = state.file_report.rev;
    let id = NodeSecret::from_bytes(&[1u8; 32]).peer_id();
    let body = put_body(
        rev,
        serde_json::json!({"route": {"exits": [id.to_string()]}}),
    );
    let resp = state.put(&body, &node);
    let (status, _v) = body_json(resp);
    assert_eq!(status, StatusCode::OK);
    assert_eq!(state.file_settings.net.route_exits, vec![id]);
    assert_eq!(
        state.page_config_document()["route"]["exits"],
        serde_json::json!([id.to_string()]),
        "GET returns the same list"
    );
    let applied = node.applied.lock().unwrap();
    assert_eq!(applied.len(), 1);
    assert_eq!(applied[0], vec!["route.exits".to_string()]);
    assert!(state.pending_restart_paths().is_empty());
    cleanup(&dir);
}

#[test]
fn put_blank_name_gets_random_name() {
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let node = StubNode::new();
    let rev = state.file_report.rev;
    let body = put_body(rev, serde_json::json!({"node": {"name": "  "}}));
    let resp = state.put(&body, &node);
    let (status, _v) = body_json(resp);
    assert_eq!(
        status,
        StatusCode::OK,
        "an empty name is not a refusal but a replacement"
    );
    let name = state.page_config_document()["node"]["name"]
        .as_str()
        .expect("node name in GET")
        .to_string();
    let parts: Vec<&str> = name.split('-').collect();
    assert_eq!(parts.len(), 2, "{name}");
    assert!(crate::members::words::is_tail_word(parts[0]), "{name}");
    assert!(crate::members::words::is_tail_word(parts[1]), "{name}");
    assert_eq!(
        state.file_settings.net.node_name.as_deref(),
        Some(name.as_str()),
        "the same name must land in the file"
    );
    cleanup(&dir);
}

#[test]
fn put_null_name_removes_file_name() {
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let node = StubNode::new();
    let rev1 = state.file_report.rev;
    let set_name = put_body(rev1, serde_json::json!({"node": {"name": "a"}}));
    let (status1, _v1) = body_json(state.put(&set_name, &node));
    assert_eq!(status1, StatusCode::OK, "name 'a' is valid");

    let rev2 = state.file_report.rev;
    let clear_name = put_body(rev2, serde_json::json!({"node": {"name": null}}));
    let (status2, _v2) = body_json(state.put(&clear_name, &node));
    assert_eq!(status2, StatusCode::OK);
    assert_eq!(
        state.page_config_document()["node"]["name"],
        Value::Null,
        "null removes the name from the file"
    );
    cleanup(&dir);
}

#[test]
fn put_route_exits_bad_id_is_invalid() {
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let node = StubNode::new();
    let rev = state.file_report.rev;
    let body = put_body(rev, serde_json::json!({"route": {"exits": ["bad"]}}));
    let resp = state.put(&body, &node);
    let (status, v) = body_json(resp);
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(v["error"], "invalid");
    let fields = v["fields"].as_array().unwrap();
    assert!(fields.iter().any(|f| {
        f["path"] == "route.exits" && f["message"] == "a list of node ids without repeats, up to 64"
    }));
    cleanup(&dir);
}

#[test]
fn put_web_access_is_written_and_not_restart_pending() {
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let node = StubNode::new();
    let rev = state.file_report.rev;
    let body = put_body(rev, serde_json::json!({"web": {"access": "localhost"}}));
    let (status, v) = body_json(state.put(&body, &node));
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        v["changed"].as_array().unwrap(),
        &vec![Value::String("web.access".to_string())]
    );
    assert_eq!(
        state.file_settings.process.web_access,
        crate::config::config_file::ListenAccess::Localhost
    );
    assert!(state.pending_restart_paths().is_empty());
    cleanup(&dir);
}

#[test]
fn put_without_web_access_keeps_file_value() {
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let node = StubNode::new();
    let rev = state.file_report.rev;
    let body = put_body(rev, serde_json::json!({"web": {"access": "localhost"}}));
    assert_eq!(body_json(state.put(&body, &node)).0, StatusCode::OK);
    let rev = state.file_report.rev;
    let body = put_body(rev, serde_json::json!({"probe": {"interval_sec": 90}}));
    assert_eq!(body_json(state.put(&body, &node)).0, StatusCode::OK);
    assert_eq!(
        state.file_settings.process.web_access,
        crate::config::config_file::ListenAccess::Localhost
    );
    cleanup(&dir);
}

#[test]
fn put_keeps_net() {
    // given: a file with net.sync = "mem"
    let dir = temp_dir();
    let mut state = new_state_with_net_sync(&dir, "mem");
    let node = StubNode::new();
    let rev = state.file_report.rev;
    // when: a PUT with a patch that does not touch the net section but rewrites the file
    let body = put_body(
        rev,
        serde_json::json!({"transport": {"mode": "relay_only"}}),
    );
    let (status, v) = body_json(state.put(&body, &node));
    // then: the file is rewritten, and net.sync in it is still "mem"
    assert_eq!(status, StatusCode::OK);
    assert!(v["changed"]
        .as_array()
        .unwrap()
        .contains(&Value::String("transport.mode".to_string())));
    assert_eq!(net_sync_in_file(&dir), "mem");
    cleanup(&dir);
}

#[test]
fn put_rejects_bad_net() {
    // given: a file from the new state, a PUT with an invalid sync name
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let node = StubNode::new();
    let rev = state.file_report.rev;
    let before = std::fs::read(dir.join("config.json")).unwrap();
    let body = put_body(rev, serde_json::json!({"net": {"sync": "foo"}}));
    // when: PUT
    let (status, v) = body_json(state.put(&body, &node));
    // then: 422 with the net.sync path, like any field error; the file is not rewritten, the node is untouched
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(v["error"], "invalid");
    let fields = v["fields"].as_array().unwrap();
    assert!(
        fields.iter().any(|f| f["path"] == "net.sync"),
        "no net.sync error: {fields:?}"
    );
    assert_eq!(
        std::fs::read(dir.join("config.json")).unwrap(),
        before,
        "file rewritten"
    );
    assert!(node.applied.lock().unwrap().is_empty());
    cleanup(&dir);
}

mod retired_key {
    use super::*;

    #[test]
    fn put_skips_it_silently_and_keeps_it_out_of_file() {
        // given: a fresh page state
        let dir = temp_dir();
        let mut state = new_state(&dir);
        let rev = state.file_report.rev;
        // when: a PUT with the node name and the removed key `transport.udp_port` (earlier 70000 gave 422)
        let body = put_body(
            rev,
            serde_json::json!({"node": {"name": "alpha"}, "transport": {"udp_port": 70000}}),
        );
        let (status, v) = body_json(state.put(&body, &StubNode::new()));
        let text = config_text(&dir);
        cleanup(&dir);
        // then: 200, only the name changed, and the removed key is absent from the file
        assert_eq!(status, StatusCode::OK, "{v}");
        assert_eq!(v["changed"], serde_json::json!(["node.name"]), "{v}");
        assert!(text.contains("alpha"), "name not written: {text}");
        assert!(
            !text.contains("udp_port"),
            "the removed key ended up in the file: {text}"
        );
    }
}

/// Page state over a file in which `net.sync` is already written with the value `sync`.
fn new_state_with_net_sync(dir: &Path, sync: &str) -> ConfigState {
    let net = crate::transport::net::NetChoice {
        sync: sync.to_string(),
        ..Settings::default().net.net_choice
    };
    let settings = Settings {
        net: crate::config::config_file::NetSettings {
            net_choice: net,
            ..crate::config::config_file::NetSettings::default()
        },
        ..Settings::default()
    };
    let secret = NodeSecret::from_bytes(&[7u8; 32]);
    config_file::save_with_floor(&dir.join("config.json"), None, 0, 0, &secret, &settings)
        .expect("write a file with the net section");
    new_state(dir)
}

/// `net.sync` as written in the config file on disk.
fn net_sync_in_file(dir: &Path) -> String {
    match config_file::load_config_file(&dir.join("config.json")) {
        LoadedConfigFile::Parsed(p) => p.settings.net.net_choice.sync,
        other => panic!("expected ParsedConfigFile, got {other:?}"),
    }
}

/// The text of the config file on disk.
fn config_text(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("config.json")).expect("config file on disk")
}
