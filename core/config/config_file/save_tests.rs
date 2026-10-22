use super::*;
use crate::config::config_file::settings::Settings;
use crate::config::config_file::test_support::{
    clear_readonly, names_in, parsed, sample_settings, scratch, test_secret, zero_secret_b64,
};
use crate::config::config_file::{SecretRead, FILE_NAME};

#[test]
fn save_then_load_roundtrip() {
    let dir = scratch("roundtrip");
    let file = dir.join(FILE_NAME);
    let s = sample_settings();
    let rev = save_config_file(&file, None, 1_790_278_867, &test_secret(), &s).expect("save");
    assert_eq!(rev, 1);
    let p = parsed(&file);
    assert_eq!(p.rev, 1);
    match &p.secret {
        SecretRead::Valid(k) => assert_eq!(k.to_bytes(), test_secret().to_bytes()),
        other => panic!("expected Valid, got {other:?}"),
    }
    assert_eq!(p.settings, s);
    assert_eq!(p.field_errors, vec![]);
    assert!(
        p.unknown_paths.is_empty(),
        "unknown fields: {:?}",
        p.unknown_paths
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_bumps_rev() {
    let dir = scratch("bumps");
    let file = dir.join(FILE_NAME);
    let s = Settings::default();
    assert_eq!(
        save_config_file(&file, None, 1, &test_secret(), &s).expect("save"),
        1
    );
    assert_eq!(
        save_config_file(&file, Some(1), 2, &test_secret(), &s).expect("save"),
        2
    );
    assert_eq!(parsed(&file).rev, 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_stale_rev() {
    let dir = scratch("stale");
    let file = dir.join(FILE_NAME);
    let s = Settings::default();
    assert_eq!(
        save_config_file(&file, None, 1, &test_secret(), &s).expect("save"),
        1
    );
    let got = save_config_file(&file, Some(5), 2, &test_secret(), &s);
    assert!(matches!(got, Err(SaveError::StaleRev(1))), "got {got:?}");
    assert_eq!(parsed(&file).rev, 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The revision is at the `u64` limit: the next one cannot be issued, and `save_config_file` answers with an error (not a panic in
/// a debug build or a wrap to zero in a release one), and the file on disk is left untouched.
#[test]
fn rev_overflow_is_error_not_panic() {
    let dir = scratch("rev-overflow");
    let file = dir.join(FILE_NAME);
    let text = format!(
        r#"{{"rev": {}, "identity": {{"secret": "{}"}}, "config": {{"version": 1}}}}"#,
        u64::MAX,
        zero_secret_b64()
    );
    std::fs::write(&file, &text).expect("writing the test file");
    assert_eq!(parsed(&file).rev, u64::MAX);
    let before = std::fs::read(&file).expect("reading before");
    let got = save_config_file(
        &file,
        Some(u64::MAX),
        1,
        &test_secret(),
        &Settings::default(),
    );
    let after = std::fs::read(&file).expect("reading after");
    let names = names_in(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(matches!(got, Err(SaveError::Io(_))), "got {got:?}");
    assert_eq!(after, before, "the file on disk changed");
    assert_eq!(
        names,
        vec![FILE_NAME.to_string()],
        "the directory has extra files"
    );
}

/// The file vanished or got corrupted between reading and writing: `save_config_file` with an expected revision does not write.
#[test]
fn save_stale_when_file_vanished_or_corrupt() {
    let dir = scratch("stale-vanished");
    let file = dir.join(FILE_NAME);
    let s = Settings::default();
    // (a) no file: the revision on disk is 0.
    let got = save_config_file(&file, Some(3), 1, &test_secret(), &s);
    let exists = file.exists();
    assert!(matches!(got, Err(SaveError::StaleRev(0))), "got {got:?}");
    assert!(!exists, "the file appeared although nothing was written");
    // (b) the file exists but does not parse: that is `Io`, and the file stays as it was.
    std::fs::write(&file, "{").expect("writing the test file");
    let got = save_config_file(&file, Some(3), 2, &test_secret(), &s);
    let bytes = std::fs::read(&file).expect("reading");
    let names = names_in(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(matches!(got, Err(SaveError::Io(_))), "got {got:?}");
    assert_eq!(bytes, b"{", "the file on disk changed");
    assert_eq!(
        names,
        vec![FILE_NAME.to_string()],
        "the directory has extra files"
    );
}

#[cfg(unix)]
#[test]
fn save_permissions_unix() {
    use std::os::unix::fs::PermissionsExt;
    let root = scratch("perms");
    // The `sub` directory does not exist: `save_config_file` creates it, and it must get 0700.
    let file = root.join("sub").join(FILE_NAME);
    save_config_file(&file, None, 1, &test_secret(), &Settings::default()).expect("save");
    let file_mode = std::fs::metadata(&file).expect("file").permissions().mode() & 0o777;
    let dir_mode = std::fs::metadata(root.join("sub"))
        .expect("directory")
        .permissions()
        .mode()
        & 0o777;
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(file_mode, 0o600, "file mode {file_mode:o}");
    assert_eq!(dir_mode, 0o700, "directory mode {dir_mode:o}");
}

#[test]
fn save_leaves_no_tmp() {
    let dir = scratch("notmp");
    let file = dir.join(FILE_NAME);
    save_config_file(&file, None, 1, &test_secret(), &Settings::default()).expect("save");
    let names: Vec<String> = std::fs::read_dir(&dir)
        .expect("directory")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(names, vec![FILE_NAME.to_string()]);
}

#[cfg(unix)]
#[test]
fn save_readonly_dir_fails() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("readonly");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500))
        .expect("setting the test directory mode");
    // Running as root, directory permissions don't prevent writing: nothing to check.
    let probe = dir.join("probe");
    if std::fs::write(&probe, b"x").is_ok() {
        let _ = std::fs::remove_file(&probe);
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    let file = dir.join(FILE_NAME);
    let got = save_config_file(&file, None, 1, &test_secret(), &Settings::default());
    let names: Vec<String> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(matches!(got, Err(SaveError::Io(_))), "got {got:?}");
    assert!(
        names.is_empty(),
        "files are left in the directory: {names:?}"
    );
}

#[test]
fn unknown_fields_dropped_on_write() {
    let dir = scratch("unknown");
    let file = dir.join(FILE_NAME);
    let text = format!(
        r#"{{"rev": 4, "identity": {{"secret": "{}"}}, "config": {{"version": 1, "zzz": 1}}}}"#,
        zero_secret_b64()
    );
    std::fs::write(&file, text).expect("writing the test file");
    let p = parsed(&file);
    assert_eq!(p.unknown_paths, vec!["config.zzz"]);
    save_config_file(&file, Some(p.rev), 1, &test_secret(), &p.settings).expect("save");
    let after = std::fs::read_to_string(&file).expect("reading");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        !after.contains("zzz"),
        "the unknown field survived: {after}"
    );
}

#[test]
fn passthrough_fields_survive() {
    let dir = scratch("passthrough");
    let file = dir.join(FILE_NAME);
    let text = format!(
        r#"{{"rev": 2, "identity": {{"secret": "{}"}}, "config": {{"version": 1,
        "node": {{"name": "ginger-pool"}},
        "web": {{"enabled": false, "port": 8080}},
        "log": {{"ring_lines": 500}}}}}}"#,
        zero_secret_b64()
    );
    std::fs::write(&file, text).expect("writing the test file");
    let first = parsed(&file);
    assert_eq!(first.field_errors, vec![]);
    save_config_file(&file, Some(first.rev), 1, &test_secret(), &first.settings).expect("save");
    let second = parsed(&file);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(second.rev, 3);
    assert_eq!(second.settings.process.web_port, 8080);
    assert!(!second.settings.process.web_enabled);
    assert_eq!(second.settings.process.log_ring_lines, 500);
    assert_eq!(
        second.settings.net.node_name.as_deref(),
        Some("ginger-pool")
    );
}

#[test]
fn set_aside_bad_renames() {
    let dir = scratch("aside");
    let file = dir.join(FILE_NAME);
    std::fs::write(&file, "{ broken").expect("writing the test file");
    let moved = set_aside_bad(&file, 1_790_278_867).expect("set_aside_bad");
    let old_exists = file.exists();
    let new_exists = moved.exists();
    let name = moved
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!old_exists, "config.json stayed in place");
    assert!(new_exists, "the set-aside file is missing");
    assert_eq!(name, "config.json.bad-20260924-194107");
}

#[test]
fn save_readonly_file_fails() {
    let dir = scratch("readonly-file");
    let file = dir.join(FILE_NAME);
    save_config_file(&file, None, 1, &test_secret(), &Settings::default())
        .expect("saving the original file");
    let mut perms = std::fs::metadata(&file).expect("metadata").permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&file, perms).expect("readonly");
    let got = save_config_file(&file, None, 2, &test_secret(), &Settings::default());
    let names = names_in(&dir);
    clear_readonly(&file);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(matches!(got, Err(SaveError::Io(_))), "got {got:?}");
    assert_eq!(
        names,
        vec![FILE_NAME.to_string()],
        "extra files appeared in the directory: {names:?}"
    );
}

/// `save_with_floor(path, None, floor, …)` gives `floor + 1`; `save_config_file` still gives `1`.
#[test]
fn save_with_floor_uses_floor() {
    let dir = scratch("floor");
    let file = dir.join(FILE_NAME);
    let got = save_with_floor(&file, None, 5, 1, &test_secret(), &Settings::default());
    let _ = std::fs::remove_dir_all(&dir);
    assert!(matches!(got, Ok(6)), "got {got:?}");

    let dir2 = scratch("floor-save-plain");
    let file2 = dir2.join(FILE_NAME);
    let got2 = save_config_file(&file2, None, 1, &test_secret(), &Settings::default());
    let _ = std::fs::remove_dir_all(&dir2);
    assert!(matches!(got2, Ok(1)), "got {got2:?}");
}
