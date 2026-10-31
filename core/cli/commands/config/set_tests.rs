use serde_json::json;

use super::*;
use crate::config::config_file::{self, LoadedConfigFile, Settings};

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("config_set_tests-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn field(path: &str, value: &str) -> FieldSet {
    FieldSet::check(path, value_of(value)).unwrap()
}

fn parsed(path: &std::path::Path) -> config_file::ParsedConfigFile {
    match config_file::load_config_file(path) {
        LoadedConfigFile::Parsed(p) => p,
        _ => panic!("the file did not parse"),
    }
}

#[test]
fn offline_writes_field_and_bumps_rev() {
    let dir = scratch("write");
    let path = dir.join("config.json");
    let secret = config_file::new_secret().unwrap();
    let mut before = Settings::default();
    before.net.node_name = Some("keep".to_string());
    config_file::save_config_file(&path, None, 1, &secret, &before).unwrap();
    save(&path, &field("probe.interval_sec", "20")).unwrap();
    let p = parsed(&path);
    assert_eq!(p.rev, 2);
    assert_eq!(p.settings.net.node_name.as_deref(), Some("keep"));
    let mut expected = before;
    field("probe.interval_sec", "20").apply_to(&mut expected);
    assert_eq!(p.settings, expected);
    assert!(matches!(p.secret, config_file::SecretRead::Valid(_)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn offline_absent_creates_file() {
    let dir = scratch("absent");
    let path = dir.join("config.json");
    save(&path, &field("probe.interval_sec", "20")).unwrap();
    let p = parsed(&path);
    assert_eq!(p.rev, 1);
    let mut expected = p.settings.clone();
    field("probe.interval_sec", "20").apply_to(&mut expected);
    assert_eq!(p.settings, expected);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn offline_invalid_file_untouched() {
    let dir = scratch("invalid");
    let path = dir.join("config.json");
    std::fs::write(&path, "{ broken").unwrap();
    assert!(save(&path, &field("probe.interval_sec", "20")).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"{ broken");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn offline_file_only_allowed() {
    let dir = scratch("file-only");
    let path = dir.join("config.json");
    let set = field("probe.full_cycle", "true");
    assert!(set.is_file_only());
    save(&path, &set).unwrap();
    assert!(parsed(&path).settings.net.probe_full_cycle);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn put_body_is_one_section() {
    assert_eq!(
        patch(&field("probe.interval_sec", "20")),
        json!({"probe": {"interval_sec": 20}})
    );
}

#[test]
fn file_only_refused_on_live_node() {
    let why = live_refusal(
        &field("probe.full_cycle", "true"),
        &json!({"read_only": []}),
    );
    assert!(why
        .unwrap()
        .contains("stop Mesh Town, then run config set again"));
}

#[test]
fn read_only_from_get_refused() {
    let current = json!({"read_only": ["probe.interval_sec"]});
    let why = live_refusal(&field("probe.interval_sec", "20"), &current);
    assert!(why
        .unwrap()
        .contains("shared by all networks; run without --net"));
    assert!(live_refusal(
        &field("probe.interval_sec", "20"),
        &json!({"read_only": []})
    )
    .is_none());
}
