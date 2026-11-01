//! `PUT /api/config` keeps the per-network relay mode when the patch does not touch the `net` section.

use super::*;
use crate::config::config_file::RelayMode;
use crate::transport::net::NodeSecret;

#[test]
fn put_keeps_net_relay() {
    // given: a file with net.relay = "off"
    let dir = temp_dir();
    let mut state = new_state_with_net_relay(&dir, RelayMode::Off);
    let node = StubNode::new();
    let rev = state.file_report.rev;
    // when: a PUT with a patch that does not touch the net section but rewrites the file
    let body = put_body(rev, serde_json::json!({"exit": {"enabled": false}}));
    let (status, v) = body_json(state.put(&body, &node));
    // then: the file is rewritten, and net.relay in it is still "off"
    assert_eq!(status, StatusCode::OK);
    assert!(v["changed"]
        .as_array()
        .unwrap()
        .contains(&Value::String("exit.enabled".to_string())));
    assert_eq!(
        net_relay_in_file(&dir),
        RelayMode::Off,
        "net.relay lost from the file: {}",
        std::fs::read_to_string(dir.join("config.json")).unwrap_or_default()
    );
    cleanup(&dir);
}

#[test]
fn put_net_relay_is_pending_a_restart() {
    // given: a default state and a node that records what it is asked to apply
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let node = StubNode::new();
    let rev = state.file_report.rev;
    // when: the relay mode is changed
    let body = put_body(rev, serde_json::json!({"net": {"relay": "off"}}));
    let (status, v) = body_json(state.put(&body, &node));
    // then: the field is saved, waits for a restart and is not applied live
    assert_eq!(status, StatusCode::OK);
    assert!(v["changed"]
        .as_array()
        .unwrap()
        .contains(&Value::String("net.relay".to_string())));
    assert_eq!(v["restart_pending"], serde_json::json!(["net.relay"]));
    assert!(state.config_get_body()["restart_fields"]
        .as_array()
        .unwrap()
        .contains(&Value::String("net.relay".to_string())));
    let applied = node.applied.lock().unwrap().clone();
    assert!(
        applied.iter().flatten().all(|p| p != "net.relay"),
        "{applied:?}"
    );
    assert_eq!(net_relay_in_file(&dir), RelayMode::Off);
    cleanup(&dir);
}

#[test]
fn net_relay_pending_ends_with_the_restart() {
    // given: a saved relay mode that waits for a restart of the lap "aaaaaaaa"
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let node = crate::web_gui::test_host::SnapNode::new(NodeSnapshot::default());
    node.snapshot.lock().unwrap().boot_id = "aaaaaaaa".to_string();
    state.sync_with_node(&*node);
    let rev = state.file_report.rev;
    let body = put_body(rev, serde_json::json!({"net": {"relay": "off"}}));
    let (_, v) = body_json(state.put(&body, &*node));
    assert_eq!(v["restart_pending"], serde_json::json!(["net.relay"]));
    // when: the node starts a new lap
    node.snapshot.lock().unwrap().boot_id = "bbbbbbbb".to_string();
    state.sync_with_node(&*node);
    // then: nothing is pending any more
    assert!(state.pending_restart_paths().is_empty());
    cleanup(&dir);
}

/// Page state over a file in which `net.relay` is already written with the value `relay`.
fn new_state_with_net_relay(dir: &Path, relay: RelayMode) -> ConfigState {
    let settings = Settings {
        net: crate::config::config_file::NetSettings {
            relay,
            ..crate::config::config_file::NetSettings::default()
        },
        ..Settings::default()
    };
    let secret = NodeSecret::from_bytes(&[7u8; 32]);
    config_file::save_with_floor(&dir.join("config.json"), None, 0, 0, &secret, &settings)
        .expect("write a file with the relay field");
    new_state(dir)
}

/// `net.relay` as read back from the config file on disk.
fn net_relay_in_file(dir: &Path) -> RelayMode {
    match config_file::load_config_file(&dir.join("config.json")) {
        LoadedConfigFile::Parsed(p) => p.settings.net.relay,
        other => panic!("expected ParsedConfigFile, got {other:?}"),
    }
}
