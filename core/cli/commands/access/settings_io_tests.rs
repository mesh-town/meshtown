use super::*;
use crate::config::config_file::{self, ListenAccess};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("settings_io_tests-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_default(path: &Path) {
    let secret = config_file::new_secret().unwrap();
    config_file::save_config_file(path, None, 1, &secret, &Settings::default()).unwrap();
}

#[test]
fn edit_file_bumps_rev_and_keeps_other_fields() {
    let dir = scratch("bump");
    let path = dir.join("config.json");
    write_default(&path);
    edit_file(&path, "access", |s| {
        s.process.web_access = ListenAccess::Localhost
    })
    .unwrap();
    let LoadedConfigFile::Parsed(p) = config_file::load_config_file(&path) else {
        panic!("the file did not parse")
    };
    assert_eq!(p.rev, 2);
    let mut expected = Settings::default();
    expected.process.web_access = ListenAccess::Localhost;
    assert_eq!(p.settings, expected);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn edit_file_without_file_creates_nothing() {
    let dir = scratch("absent");
    let path = dir.join("config.json");
    let e = edit_file(&path, "access", |_| {}).unwrap_err();
    assert!(e.to_string().contains("no settings file"), "{e}");
    assert!(e.to_string().contains("change the access"), "{e}");
    assert!(!path.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn edit_file_refuses_invalid_file_and_keeps_bytes() {
    let dir = scratch("invalid");
    let path = dir.join("config.json");
    std::fs::write(&path, "not json").unwrap();
    let e = edit_file(&path, "access", |_| panic!("must not run")).unwrap_err();
    assert!(e.to_string().contains("settings file is unusable"), "{e}");
    assert_eq!(std::fs::read(&path).unwrap(), b"not json");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn edit_file_refuses_missing_secret() {
    let dir = scratch("secret");
    let path = dir.join("config.json");
    write_default(&path);
    let mut doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    doc.as_object_mut()
        .unwrap()
        .retain(|k, _| k == "config" || k == "rev");
    std::fs::write(&path, doc.to_string()).unwrap();
    let before = std::fs::read(&path).unwrap();
    let e = edit_file(&path, "access", |s| {
        s.process.web_access = ListenAccess::Localhost
    });
    let text = e.unwrap_err().to_string();
    assert!(
        text.contains("no valid key") || text.contains("unusable"),
        "{text}"
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let _ = std::fs::remove_dir_all(&dir);
}
