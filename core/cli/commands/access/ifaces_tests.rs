use super::*;
use crate::config::config_file::{LoadedConfigFile, SecretRead, Settings};

fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ifaces_tests-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn target(dir: &std::path::Path) -> std::path::PathBuf {
    dir.join("config.json")
}

#[test]
fn names_parse_to_filters() {
    assert_eq!(parse_names(&names(&["auto"])), Ok(IfaceFilter::Auto));
    assert_eq!(parse_names(&names(&["*"])), Ok(IfaceFilter::Any));
    assert_eq!(
        parse_names(&names(&["br-lan"])),
        Ok(IfaceFilter::Only(names(&["br-lan"])))
    );
}

#[test]
fn bad_names_are_refused_with_expected_text() {
    for bad in [&["auto", "x"][..], &["br-lan", "br-lan"], &["*", "x"]] {
        assert_eq!(
            parse_names(&names(bad)),
            Err(EXPECTED.to_string()),
            "{bad:?}"
        );
    }
}

fn text(t: &str) -> FileRead {
    FileRead::Text(t.to_string())
}

#[test]
fn seed_skips_with_any_key() {
    let socks = r#"{"config":{"version":1,"socks":{"ifaces":[]}}}"#;
    let web = r#"{"config":{"version":1,"web":{"ifaces":["x"]}}}"#;
    for t in [socks, web] {
        assert!(
            matches!(seed_decision(true, &text(t)), Seed::Skip(_)),
            "{t}"
        );
    }
}

#[test]
fn seed_creates_when_file_absent() {
    assert_eq!(seed_decision(true, &FileRead::Absent), Seed::Create);
}

#[test]
fn seed_skips_unreadable() {
    assert_eq!(
        seed_decision(true, &FileRead::Unreadable),
        Seed::Skip("settings file is unreadable")
    );
}

#[test]
fn seed_writes_when_no_keys_and_always_without_flag() {
    let bare = r#"{"config":{"version":1,"socks":{"port":1080}}}"#;
    assert_eq!(seed_decision(true, &text(bare)), Seed::Write);
    assert_eq!(seed_decision(false, &FileRead::Absent), Seed::Write);
    assert_eq!(
        seed_decision(false, &text(r#"{"config":{"socks":{"ifaces":[]}}}"#)),
        Seed::Write
    );
}

#[test]
fn create_writes_file_with_names_and_defaults() {
    let dir = scratch("create");
    let t = dir.join("sub").join("config.json");
    let filter = IfaceFilter::Only(names(&["br-lan"]));
    create_seeded(&t, &filter).unwrap();
    let LoadedConfigFile::Parsed(p) = config_file::load_config_file(&t) else {
        panic!("the file did not parse")
    };
    assert_eq!(p.rev, 1);
    assert!(matches!(p.secret, SecretRead::Valid(_)));
    let mut settings = p.settings;
    assert_eq!(settings.process.socks_ifaces, filter);
    assert_eq!(settings.process.web_ifaces, filter);
    let name = settings.net.node_name.take().unwrap();
    assert_eq!(name.split('-').count(), 2, "{name}");
    settings.process.socks_ifaces = IfaceFilter::Auto;
    settings.process.web_ifaces = IfaceFilter::Auto;
    assert_eq!(settings, Settings::default());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&t), 0o600);
        assert_eq!(mode(t.parent().unwrap()), 0o700);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn create_refuses_existing_file() {
    let dir = scratch("create-exists");
    let t = target(&dir);
    std::fs::write(&t, "not json").unwrap();
    assert!(create_seeded(&t, &IfaceFilter::Any).is_err());
    assert_eq!(std::fs::read_to_string(&t).unwrap(), "not json");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn create_skipped_when_empty_list_present() {
    let dir = scratch("create-skip");
    let t = target(&dir);
    let secret = config_file::new_secret().unwrap();
    config_file::save_config_file(&t, None, 1, &secret, &Settings::default()).unwrap();
    let before = std::fs::read(&t).unwrap();
    let read = read_file(&t);
    assert!(matches!(seed_decision(true, &read), Seed::Skip(_)));
    assert_eq!(std::fs::read(&t).unwrap(), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn create_seeds_existing_file_without_keys() {
    let dir = scratch("create-nokeys");
    let t = target(&dir);
    let secret = config_file::new_secret().unwrap();
    config_file::save_config_file(&t, None, 1, &secret, &Settings::default()).unwrap();
    let mut doc: Value = serde_json::from_str(&std::fs::read_to_string(&t).unwrap()).unwrap();
    for section in ["socks", "web"] {
        doc["config"][section]
            .as_object_mut()
            .unwrap()
            .remove("ifaces");
    }
    std::fs::write(&t, doc.to_string()).unwrap();
    let filter = IfaceFilter::Only(names(&["br-lan"]));
    assert_eq!(seed_decision(true, &read_file(&t)), Seed::Write);
    offline(&t, &filter).unwrap();
    let LoadedConfigFile::Parsed(p) = config_file::load_config_file(&t) else {
        panic!("the file did not parse")
    };
    assert_eq!(p.settings.process.socks_ifaces, filter);
    assert_eq!(p.settings.process.web_ifaces, filter);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn offline_writes_both_fields_and_bumps_rev() {
    let dir = scratch("offline");
    let t = target(&dir);
    let secret = config_file::new_secret().unwrap();
    config_file::save_config_file(&t, None, 1, &secret, &Settings::default()).unwrap();
    let filter = IfaceFilter::Only(names(&["br-lan"]));
    offline(&t, &filter).unwrap();
    let LoadedConfigFile::Parsed(p) = config_file::load_config_file(&t) else {
        panic!("the file did not parse")
    };
    assert_eq!(p.rev, 2);
    assert_eq!(p.settings.process.socks_ifaces, filter);
    assert_eq!(p.settings.process.web_ifaces, filter);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn offline_without_file_creates_nothing() {
    let dir = scratch("absent");
    let t = target(&dir);
    assert!(offline(&t, &IfaceFilter::Any).is_err());
    assert!(!t.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn live_save_mentions_restart_only_when_pending() {
    let filter = IfaceFilter::Only(names(&["br-lan"]));
    let pending = json!({"restart_pending": ["web.ifaces"]});
    assert!(restart_needed(&pending));
    assert!(saved_text(&pending, &filter).contains("restarts"));
    for none in [
        json!({"restart_pending": []}),
        json!({}),
        json!({"restart_pending": ["web.access"]}),
    ] {
        assert!(!restart_needed(&none), "{none}");
        let text = saved_text(&none, &filter);
        assert!(!text.contains("restart"), "{text}");
        assert!(text.starts_with("Interface list saved: "), "{text}");
    }
}
