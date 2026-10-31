use super::*;
use crate::config::config_file::{self, ListenAccess, LoadedConfigFile};

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
    assert!(text.contains("no valid key"), "{text}");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let _ = std::fs::remove_dir_all(&dir);
}

fn parsed(path: &Path) -> config_file::ParsedConfigFile {
    match config_file::load_config_file(path) {
        LoadedConfigFile::Parsed(p) => p,
        _ => panic!("the file did not parse"),
    }
}

fn secret_of(p: &config_file::ParsedConfigFile) -> [u8; 32] {
    match &p.secret {
        config_file::SecretRead::Valid(s) => s.to_bytes(),
        _ => panic!("no valid key"),
    }
}

/// Writes the file as if another process saved it between our read and our write.
fn foreign_write(path: &Path, name: &str) {
    let p = parsed(path);
    let config_file::SecretRead::Valid(secret) = &p.secret else {
        panic!("no valid key")
    };
    let mut s = p.settings.clone();
    s.net.node_name = Some(name.to_string());
    config_file::save_config_file(path, Some(p.rev), 5, secret, &s).unwrap();
}

#[test]
fn edit_file_redoes_a_write_that_lost_the_race() {
    let dir = scratch("retry");
    let path = dir.join("config.json");
    write_default(&path);
    let before = secret_of(&parsed(&path));
    let rev0 = parsed(&path).rev;
    let mut first = true;
    edit_file(&path, "settings", |s| {
        if std::mem::take(&mut first) {
            foreign_write(&path, "other");
        }
        s.net.probe_interval_sec = 77;
    })
    .unwrap();
    let p = parsed(&path);
    assert_eq!(p.settings.net.node_name.as_deref(), Some("other"));
    assert_eq!(p.settings.net.probe_interval_sec, 77);
    assert_eq!(p.rev, rev0 + 2);
    assert_eq!(secret_of(&p), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn edit_file_gives_up_when_the_file_keeps_changing() {
    let dir = scratch("giveup");
    let path = dir.join("config.json");
    write_default(&path);
    let mut n = 0;
    let e = edit_file(&path, "settings", |_| {
        n += 1;
        foreign_write(&path, &format!("other{n}"));
    })
    .unwrap_err();
    assert!(e.to_string().contains("changed by someone else"), "{e}");
    let p = parsed(&path);
    assert_eq!(p.settings.net.node_name.as_deref(), Some("other2"));
    let _ = std::fs::remove_dir_all(&dir);
}
