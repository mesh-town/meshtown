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
