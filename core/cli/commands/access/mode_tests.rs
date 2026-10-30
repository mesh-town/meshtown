use super::*;
use crate::config::config_file::{self, LoadedConfigFile};

#[test]
fn patch_touches_only_its_field() {
    assert_eq!(
        patch(&PAGE, ListenAccess::Localnet),
        json!({"web": {"access": "localnet"}})
    );
    assert_eq!(
        patch(&PROXY, ListenAccess::Localhost),
        json!({"socks": {"access": "localhost"}})
    );
}

#[test]
fn restart_needed_only_when_pending_names_field() {
    let yes = json!({"restart_pending": ["socks.access"]});
    assert!(restart_needed(&yes, &PROXY));
    let no = [
        json!({"restart_pending": []}),
        json!({}),
        json!({"restart_pending": "socks.access"}),
        json!({"restart_pending": ["web.access"]}),
    ];
    for reply in no {
        assert!(!restart_needed(&reply, &PROXY), "{reply}");
    }
}

fn scratch_file(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("mode_tests-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.json");
    let secret = config_file::new_secret().unwrap();
    config_file::save_config_file(&path, None, 1, &secret, &Settings::default()).unwrap();
    path
}

fn read(path: &std::path::Path) -> Settings {
    let LoadedConfigFile::Parsed(p) = config_file::load_config_file(path) else {
        panic!("the file did not parse")
    };
    p.settings
}

#[test]
fn offline_page_and_proxy_write_distinct_fields() {
    let path = scratch_file("distinct");
    offline(&path, &PAGE, ListenAccess::Localhost).unwrap();
    let s = read(&path);
    assert_eq!(s.process.web_access, ListenAccess::Localhost);
    assert_eq!(
        s.process.socks_access,
        Settings::default().process.socks_access
    );
    offline(&path, &PROXY, ListenAccess::Localnet).unwrap();
    let s = read(&path);
    assert_eq!(s.process.web_access, ListenAccess::Localhost);
    assert_eq!(s.process.socks_access, ListenAccess::Localnet);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn same_mode_twice_is_ok() {
    let path = scratch_file("twice");
    offline(&path, &PROXY, ListenAccess::Localnet).unwrap();
    offline(&path, &PROXY, ListenAccess::Localnet).unwrap();
    assert_eq!(read(&path).process.socks_access, ListenAccess::Localnet);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}
