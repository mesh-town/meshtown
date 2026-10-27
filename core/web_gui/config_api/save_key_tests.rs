//! Tests for `ConfigState::save_network_key`: what stays in the file after a network change.

use crate::transport::net::NodeSecret;
use std::path::{Path, PathBuf};

use crate::config::config_file::{
    self, CliOverrides, ConfigSource, LoadedConfigFile, PathRule, Settings,
};
use crate::ports_sharing::{Audience, Opened, Proto, Published};

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
    crate::members::network_key::test_net(label).text
}

fn published() -> Vec<Published> {
    vec![Published {
        proto: Proto::Tcp,
        port: 8080,
        name: "web".into(),
        audience: Audience::All,
    }]
}

fn opened() -> Vec<Opened> {
    let node = NodeSecret::from_bytes(&[3u8; 32]).peer_id();
    vec![Opened {
        proto: Proto::Tcp,
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
    state.file = None;
    assert!(state.save_network_key(&b, false).is_ok());
    assert_eq!(state.current_network_key.as_deref(), Some(b.as_str()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn drop_network_clears_key_exits_and_publications_and_renews_the_secret() {
    let dir = temp_dir();
    let mut state = state_with_file(&dir, &fresh_key("a"));
    let rev = state.file_report.rev;
    let old_secret = state.secret.to_bytes();
    let new_secret = NodeSecret::from_bytes(&[9u8; 32]);
    let written = state.drop_network(new_secret.clone(), 2, false).unwrap();
    assert_eq!(written, Some(rev + 1));
    assert_eq!(state.secret.to_bytes(), new_secret.to_bytes());
    assert_ne!(state.secret.to_bytes(), old_secret);
    assert_eq!(state.current_network_key, None);
    let on_disk = file_settings(&dir);
    assert_eq!(on_disk.net.network_key, None);
    assert!(on_disk.net.route_exits.is_empty());
    assert!(on_disk.net.ports_published.is_empty());
    assert_eq!(on_disk.net.ports_opened, opened());
    assert_eq!(state.file_settings, on_disk);
}

#[test]
fn drop_network_with_keep_file_leaves_the_file_byte_for_byte() {
    let dir = temp_dir();
    let mut state = state_with_file(&dir, &fresh_key("a"));
    let before = std::fs::read(dir.join("config.json")).unwrap();
    let written = state
        .drop_network(NodeSecret::from_bytes(&[9u8; 32]), 2, true)
        .unwrap();
    assert_eq!(written, None);
    assert_eq!(std::fs::read(dir.join("config.json")).unwrap(), before);
    assert_eq!(state.current_network_key, None);
    assert_eq!(state.file_settings.net.network_key, None);
}

mod adopt_secret {
    use super::*;
    use crate::config::config_file::SecretRead;

    fn file_secret(dir: &Path) -> NodeSecret {
        match config_file::load_config_file(&dir.join("config.json")) {
            LoadedConfigFile::Parsed(p) => match p.secret {
                SecretRead::Valid(s) => s,
                other => panic!("the file must hold a valid secret: {other:?}"),
            },
            _ => panic!("the settings file can't be read"),
        }
    }

    fn new_secret() -> NodeSecret {
        NodeSecret::from_bytes(&[77u8; 32])
    }

    #[test]
    fn adopt_secret_then_save_keeps_the_name_and_writes_the_new_secret() {
        // given: a file with a node name
        let dir = temp_dir();
        let mut state = state_with_file(&dir, &fresh_key("a"));
        let mut named = state.file_settings.clone();
        named.net.node_name = Some("kitchen".to_string());
        let path = dir.join("config.json");
        let rev = config_file::save_config_file(
            &path,
            Some(state.file_report.rev),
            2,
            &state.secret,
            &named,
        )
        .unwrap();
        state.file_settings = named;
        state.file_report.rev = rev;
        // when
        state.adopt_secret(new_secret());
        assert!(state.save_network_key(&fresh_key("b"), false).is_ok());
        // then
        assert_eq!(file_secret(&dir).peer_id(), new_secret().peer_id());
        assert_eq!(
            file_settings(&dir).net.node_name.as_deref(),
            Some("kitchen")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn adopt_secret_without_a_file_changes_memory_only() {
        // given
        let dir = temp_dir();
        let mut state = state_with_file(&dir, &fresh_key("a"));
        state.file = None;
        let before = std::fs::read(dir.join("config.json")).unwrap();
        // when
        state.adopt_secret(new_secret());
        assert!(state.save_network_key(&fresh_key("b"), false).is_ok());
        // then
        assert_eq!(state.secret.peer_id(), new_secret().peer_id());
        assert_eq!(
            std::fs::read(dir.join("config.json")).unwrap(),
            before,
            "no file was written"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn adopt_secret_survives_a_failed_write() {
        use std::os::unix::fs::PermissionsExt;
        // given: a directory the file cannot be replaced in
        let dir = temp_dir();
        let mut state = state_with_file(&dir, &fresh_key("a"));
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).unwrap();
        // when
        state.adopt_secret(new_secret());
        let saved = state.save_network_key(&fresh_key("b"), false);
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        // then: the error goes up, the memory has the key the running node has
        if saved.is_ok() {
            // running as a user that ignores directory permissions: nothing to check
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }
        assert_eq!(state.secret.peer_id(), new_secret().peer_id());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
