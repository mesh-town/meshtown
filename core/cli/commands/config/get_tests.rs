use serde_json::json;

use super::*;
use crate::config::config_file::Settings;

#[test]
fn lines_follow_fields_order_without_key() {
    let reply = json!({"rev": 3, "config": super::super::file::document(&Settings::default())});
    let lines = lines(&reply);
    let paths: Vec<&str> = lines.iter().map(|l| l.split('=').next().unwrap()).collect();
    let table: Vec<&str> = FIELDS
        .iter()
        .map(|f| f.field_path)
        .filter(|p| paths.contains(p))
        .collect();
    assert_eq!(paths, table);
    assert!(!paths.contains(&"network.key"));
    assert!(
        lines.contains(&"probe.interval_sec=300".to_string()),
        "{lines:?}"
    );
}

#[test]
fn absent_file_gives_defaults() {
    let dir = std::env::temp_dir().join(format!("config_get_tests-absent-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let reply = body(&dir.join("config.json")).unwrap();
    assert_eq!(reply["rev"], 0);
    assert_eq!(reply["config"]["probe"]["interval_sec"], 300);
    assert!(!dir.join("config.json").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn computed_path_needs_node() {
    let field = field_by_path("network.label").unwrap();
    let e = file_reply(std::path::Path::new("unused.json"), Some(field)).unwrap_err();
    assert!(
        e.to_string().contains("known only to a running node"),
        "{e}"
    );
}

#[test]
fn network_key_and_unknown_paths_are_refused() {
    let e = pick("network.key").unwrap_err();
    assert!(e.to_string().contains("network key"), "{e}");
    let e = pick("probe.interval").unwrap_err();
    assert!(e.downcast_ref::<UsageError>().is_some(), "{e}");
}
