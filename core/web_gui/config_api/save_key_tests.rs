//! Tests for `ConfigState::save_network_key`: what stays in the file after a network change.

use crate::transport::net::NodeSecret;
use std::path::{Path, PathBuf};

use crate::config::config_file::{
    self, CliOverrides, ConfigSource, LoadedConfigFile, PathRule, Settings,
};
use crate::ports_sharing::{Audience, Opened, Published};

use super::ConfigState;

fn temp_dir() -> PathBuf {
    static COUNTER: portable_atomic::AtomicU64 = portable_atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("mesh-save-key-{}-{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn fresh_key(label: &str) -> String {
    let tail = crate::members::words::generate_tail().unwrap();
    crate::members::network_key::format_key_string(Some(label), &tail)
}

fn published() -> Vec<Published> {
    vec![Published {
        port: 8080,
        name: "web".into(),
        audience: Audience::All,
    }]
}

fn opened() -> Vec<Opened> {
    let node = NodeSecret::from_bytes(&[3u8; 32]).peer_id();
    vec![Opened {
        node,
        port: 80,
        local: 8081,
    }]
}

/// A state with a file: network key A, `ports.published = [8080 All "web"]`, one `opened`.
fn state_with_file(dir: &Path, key_a: &str) -> ConfigState {
    let path = dir.join("config.json");
    let source = ConfigSource::File {
        path: path.clone(),
        may_create: true,
        rule: PathRule::Explicit,
    };
    let run = config_file::prepare(&source, &CliOverrides::default(), true, 0).unwrap();
    let mut state = ConfigState::from_run(&run);
    let mut s = state.file_settings.clone();
    s.net.network_key = Some(key_a.to_string());
    s.net.ports_published = published();
    s.net.ports_opened = opened();
    s.net.route_exits = vec![NodeSecret::from_bytes(&[4u8; 32]).peer_id()];
    let rev =
        config_file::save_config_file(&path, Some(state.file_report.rev), 1, &state.secret, &s)
            .unwrap();
    state.file_settings = s;
    state.file_report.rev = rev;
    state.current_network_key = Some(key_a.to_string());
    state
}

fn file_settings(dir: &Path) -> Settings {
    match config_file::load_config_file(&dir.join("config.json")) {
        LoadedConfigFile::Parsed(p) => p.settings,
        _ => panic!("the settings file can't be read"),
    }
}

#[test]
fn no_clears_published_keeps_opened() {
    let dir = temp_dir();
    let (a, b) = (fresh_key("a"), fresh_key("b"));
    let mut state = state_with_file(&dir, &a);
    assert!(state.save_network_key(&b, false).is_ok());
    assert_eq!(
        state.file_settings.net.network_key.as_deref(),
        Some(b.as_str())
    );
    assert!(state.file_settings.net.ports_published.is_empty());
    assert_eq!(state.file_settings.net.ports_opened, opened());
    let on_disk = file_settings(&dir);
    assert_eq!(on_disk.net.network_key.as_deref(), Some(b.as_str()));
    assert!(on_disk.net.ports_published.is_empty());
    assert_eq!(on_disk.net.ports_opened, opened());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn switch_clears_route_exits() {
    for keep_ports in [false, true] {
        let dir = temp_dir();
        let (a, b) = (fresh_key("a"), fresh_key("b"));
        let mut state = state_with_file(&dir, &a);
        assert!(!state.file_settings.net.route_exits.is_empty());
        assert!(state.save_network_key(&b, keep_ports).is_ok());
        assert!(state.file_settings.net.route_exits.is_empty());
        assert!(file_settings(&dir).net.route_exits.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn yes_keeps_published() {
    let dir = temp_dir();
    let (a, b) = (fresh_key("a"), fresh_key("b"));
    let mut state = state_with_file(&dir, &a);
    assert!(state.save_network_key(&b, true).is_ok());
    assert_eq!(
        state.file_settings.net.network_key.as_deref(),
        Some(b.as_str())
    );
    assert_eq!(state.file_settings.net.ports_published, published());
    assert_eq!(file_settings(&dir).net.ports_published, published());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bad_file_is_set_aside_and_new_one_has_no_published() {
    let dir = temp_dir();
    let (a, b) = (fresh_key("a"), fresh_key("b"));
    let mut state = state_with_file(&dir, &a);
    std::fs::write(dir.join("config.json"), "garbage, not JSON").unwrap();
    assert!(state.save_network_key(&b, false).is_ok());
    let on_disk = file_settings(&dir);
    assert_eq!(on_disk.net.network_key.as_deref(), Some(b.as_str()));
    assert!(on_disk.net.ports_published.is_empty());
    assert!(on_disk.net.route_exits.is_empty());
    let aside = std::fs::read_dir(&dir).unwrap().count();
    assert!(
        aside >= 2,
        "the broken file must be set aside, files: {aside}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn without_file_only_current_key_changes() {
    let dir = temp_dir();
    let (a, b) = (fresh_key("a"), fresh_key("b"));
    let mut state = state_with_file(&dir, &a);
    state.config_file_path = None;
    assert!(state.save_network_key(&b, false).is_ok());
    assert_eq!(state.current_network_key.as_deref(), Some(b.as_str()));
    let _ = std::fs::remove_dir_all(&dir);
}
