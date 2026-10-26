//! Tests for the process block of an added slot: shown from the primary slot, read-only, never written.

use axum::http::StatusCode;
use http_body_util::BodyExt;
use serde_json::{json, Value};

use crate::config::config_file::fields::{self, Block};
use crate::config::config_file::{
    self, CliOverrides, ConfigSource, ListenAccess, PathRule, ProcessSettings,
};
use crate::web_gui::test_host::SnapNode;
use crate::web_gui::{ConfigState, NodeSnapshot};

fn process() -> ProcessSettings {
    ProcessSettings {
        socks_enabled: false,
        socks_port: 3333,
        web_access: ListenAccess::Localhost,
        ..ProcessSettings::default()
    }
}

fn state_in(dir: &std::path::Path) -> ConfigState {
    let source = ConfigSource::File {
        path: dir.join("config.json"),
        may_create: true,
        rule: PathRule::Explicit,
    };
    let run = config_file::prepare(&source, &CliOverrides::default(), true, 0).unwrap();
    ConfigState::from_run(&run)
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("slot-overlay-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn overlay_shows_the_given_process_block_as_read_only() {
    let dir = temp_dir("get");
    let mut state = state_in(&dir);
    state.cli_override_paths = vec!["socks.port"];
    state.overlay_process(&process());
    let body = state.config_get_body();
    assert_eq!(state.file_settings.process, process());
    assert_eq!(state.boot_file_settings.process, process());
    assert_eq!(body["cli_overrides"], json!([]));
    assert_eq!(body["restart_pending"], json!([]));
    let read_only = body["read_only"].as_array().unwrap();
    for field in fields::rows_of(Block::Process) {
        assert!(
            read_only.contains(&json!(field.field_path)),
            "{}",
            field.field_path
        );
    }
    assert_eq!(body["config"]["socks"]["port"], 3333);
    let _ = std::fs::remove_dir_all(dir);
}

mod launch_flags {
    use super::*;

    #[test]
    fn only_those_of_the_network_block_stay_listed() {
        // given: a slot state with a process flag and a network flag
        let dir = temp_dir("flags-mixed");
        let mut state = state_in(&dir);
        state.cli_override_paths = vec!["socks.port", "exit.enabled"];
        // when: the process block is overlaid
        state.overlay_process(&process());
        // then: the page still reads the network flag as "set by a flag"
        assert_eq!(
            state.config_get_body()["cli_overrides"],
            json!(["exit.enabled"])
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn process_flags_alone_leave_the_list_empty() {
        // given: a slot state with process flags only
        let dir = temp_dir("flags-process");
        let mut state = state_in(&dir);
        state.cli_override_paths = vec!["socks.port", "web.port"];
        // when: the process block is overlaid
        state.overlay_process(&process());
        // then: nothing is listed
        assert_eq!(state.config_get_body()["cli_overrides"], json!([]));
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn a_plain_state_keeps_the_table_read_only_list() {
    let dir = temp_dir("plain");
    let state = state_in(&dir);
    assert_eq!(state.read_only_list(), fields::read_only_paths());
    let _ = std::fs::remove_dir_all(dir);
}

fn put(state: &mut ConfigState, config: Value) -> (StatusCode, Value) {
    let node = SnapNode::new(NodeSnapshot::default());
    let rev = state.file_report.rev;
    let body = json!({"rev": rev, "config": config}).to_string();
    let resp = state.put(body.as_bytes(), &*node);
    let status = resp.status();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let bytes = rt.block_on(resp.into_body().collect()).unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[test]
fn put_ignores_process_fields_and_keeps_network_fields() {
    let dir = temp_dir("put");
    let mut state = state_in(&dir);
    state.current_network_key = Some("key".to_string());
    state.overlay_process(&process());
    let flipped = !state.file_settings.net.exit_enabled;
    let patch = json!({"socks": {"enabled": true, "port": 4444}, "web": {"access": "localnet"},
        "exit": {"enabled": flipped}});
    let (status, body) = put(&mut state, patch);
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(state.file_settings.process, process());
    assert_eq!(state.file_settings.net.exit_enabled, flipped);
    assert_eq!(body["changed"], json!(["exit.enabled"]));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn put_of_a_slot_without_a_network_is_busy() {
    let dir = temp_dir("busy");
    let mut state = state_in(&dir);
    state.overlay_process(&process());
    let before = state.file_settings.net.exit_enabled;
    let (status, body) = put(&mut state, json!({"exit": {"enabled": !before}}));
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"], "busy");
    assert_eq!(state.file_settings.net.exit_enabled, before);
    let _ = std::fs::remove_dir_all(dir);
}
