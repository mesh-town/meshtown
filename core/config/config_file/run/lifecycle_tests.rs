//! Settings file lifecycle scenarios: first run, repeat run, flags with
//! `--save-config`, a corrupted/newer-version file, `--no-config`, a read-only directory.
//! Moved out of `run_tests.rs` (code layout) — test bodies and names are unchanged.

use std::path::Path;

use super::*;
use crate::config::config_file::overrides::CliOverrides;
use crate::config::config_file::parse::{load_config_file, LoadedConfigFile};
use crate::config::config_file::path::{ConfigSource, PathRule};
use crate::config::config_file::report::FileState;
use crate::config::config_file::save::save_config_file;
use crate::config::config_file::settings::Settings;
use crate::config::config_file::test_support::{
    names_in, parsed, sample_settings, scratch, secret_of, test_secret,
};
use crate::config::config_file::ProcessSettings;
use crate::config::config_file::FILE_NAME;

const NOW: u64 = 1_790_278_867;

fn explicit(path: &Path) -> ConfigSource {
    ConfigSource::File {
        path: path.to_path_buf(),
        may_create: true,
        rule: PathRule::Explicit,
    }
}

#[test]
fn first_run_creates_defaults_not_flags() {
    let dir = scratch("first-run");
    // The `sub` directory does not exist: the write creates it.
    let file = dir.join("sub").join(FILE_NAME);
    let cli = CliOverrides {
        socks_port: Some(2000),
        ..CliOverrides::default()
    };
    let run = prepare(&explicit(&file), &cli, false, NOW).expect("prepare");
    let on_disk = parsed(&file);
    #[cfg(unix)]
    let file_mode = {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(&file).expect("file").permissions().mode() & 0o777
    };
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(run.file_report.state, FileState::Created);
    assert_eq!(run.file_report.rev, 1);
    assert!(run.file_report.persistent_identity);
    assert!(!run.file_report.identity_declined);
    assert_eq!(
        on_disk.settings.process.socks_port, 1080,
        "the flags landed in the file"
    );
    assert_eq!(run.file_settings.process.socks_port, 1080);
    assert_eq!(run.effective_settings.process.socks_port, 2000);
    assert_eq!(run.cli_override_paths, vec!["socks.port"]);
    assert_eq!(secret_of(&on_disk), run.secret.to_bytes());
    #[cfg(unix)]
    assert_eq!(file_mode, 0o600, "file mode {file_mode:o}");
}

#[test]
fn clean_file_without_save_config_is_only_probed() {
    let dir = scratch("clean-probed");
    let file = dir.join(FILE_NAME);
    let cli = CliOverrides::default();
    prepare(&explicit(&file), &cli, false, NOW).expect("prepare 1");
    let before = std::fs::read(&file).expect("reading the file");
    let rev = parsed(&file).rev;
    let out = super::sources::prepare_file(&file, true, &cli, false, NOW + 1).expect("prepare 2");
    let after = std::fs::read(&file).expect("reading the file again");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(out.report.writable);
    assert_eq!(out.report.rev, rev);
    assert!(!out.secret_written);
    assert_eq!(out.kept_copy_path, None);
    assert!(
        out.file_summary.ends_with(" (read)"),
        "{}",
        out.file_summary
    );
    assert_eq!(before, after);
}

#[test]
fn second_run_same_secret() {
    let dir = scratch("second-run");
    let file = dir.join(FILE_NAME);
    let cli = CliOverrides::default();
    let first = prepare(&explicit(&file), &cli, false, NOW).expect("prepare 1");
    let second = prepare(&explicit(&file), &cli, false, NOW + 1).expect("prepare 2");
    let rev_on_disk = parsed(&file).rev;
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(first.secret.to_bytes(), second.secret.to_bytes());
    assert_eq!(second.file_report.state, FileState::Ok);
    assert_eq!(second.file_report.rev, 1, "the second run wrote nothing");
    assert_eq!(rev_on_disk, 1);
    assert!(second.file_report.persistent_identity);
    assert!(!second.file_report.identity_declined);
}

#[test]
fn save_config_writes_flags() {
    let dir = scratch("save-config");
    let file = dir.join(FILE_NAME);
    let before = sample_settings();
    save_config_file(&file, None, 1, &test_secret(), &before).expect("save");
    let cli = CliOverrides {
        socks_port: Some(2000),
        ..CliOverrides::default()
    };
    let run = prepare(&explicit(&file), &cli, true, NOW).expect("prepare");
    let after = parsed(&file);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(after.rev, 2, "the revision must grow by 1");
    assert_eq!(run.file_report.rev, 2);
    assert_eq!(after.settings.process.socks_port, 2000);
    assert_eq!(
        after.settings,
        Settings {
            process: ProcessSettings {
                socks_port: 2000,
                ..before.process.clone()
            },
            ..before
        },
        "the other fields of the file changed"
    );
    assert!(
        run.cli_override_paths.is_empty(),
        "list: {:?}",
        run.cli_override_paths
    );
    assert_eq!(run.effective_settings.process.socks_port, 2000);
    assert_eq!(secret_of(&after), test_secret().to_bytes());
    assert!(!run.file_report.identity_declined);
}

#[test]
fn missing_secret_written_now() {
    let dir = scratch("missing-secret");
    let file = dir.join(FILE_NAME);
    std::fs::write(&file, r#"{"rev": 4, "config": {"version": 1}}"#).expect("write");
    let run = prepare(&explicit(&file), &CliOverrides::default(), false, NOW).expect("prepare");
    let after = parsed(&file);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(after.rev, 5);
    assert_eq!(run.file_report.rev, 5);
    assert_eq!(secret_of(&after), run.secret.to_bytes());
    assert!(run.file_report.persistent_identity);
    assert!(!run.file_report.identity_declined);
}

#[test]
fn bad_secret_replaced() {
    let dir = scratch("bad-secret");
    let file = dir.join(FILE_NAME);
    std::fs::write(
        &file,
        r#"{"rev": 1, "identity": {"secret": "!!"}, "config": {"version": 1}}"#,
    )
    .expect("write");
    let run = prepare(&explicit(&file), &CliOverrides::default(), false, NOW).expect("prepare");
    let after = parsed(&file);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(secret_of(&after), run.secret.to_bytes());
    assert_eq!(after.rev, 2);
    assert!(
        run.file_report
            .field_errors
            .iter()
            .any(|e| e.field_path == "identity.secret"),
        "field_errors: {:?}",
        run.file_report.field_errors
    );
    assert!(run.file_report.persistent_identity);
    assert!(!run.file_report.identity_declined);
}

#[test]
fn invalid_not_touched() {
    let dir = scratch("invalid");
    let file = dir.join(FILE_NAME);
    std::fs::write(&file, "{").expect("write");
    let run = prepare(&explicit(&file), &CliOverrides::default(), false, NOW).expect("prepare");
    let bytes = std::fs::read(&file).expect("read");
    let names = names_in(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(run.file_report.state, FileState::Invalid);
    assert_eq!(bytes, b"{", "the file changed");
    assert_eq!(names, vec![FILE_NAME.to_string()]);
    assert!(!run.file_report.persistent_identity);
    assert!(!run.file_report.identity_declined);
}

#[test]
fn invalid_with_save_config_sets_aside() {
    let dir = scratch("invalid-save");
    let file = dir.join(FILE_NAME);
    std::fs::write(&file, "{").expect("write");
    let run = prepare(&explicit(&file), &CliOverrides::default(), true, NOW).expect("prepare");
    let names = names_in(&dir);
    let aside = names
        .iter()
        .find(|n| n.starts_with("config.json.bad-"))
        .cloned();
    let aside_bytes = aside.as_ref().and_then(|n| std::fs::read(dir.join(n)).ok());
    let after = load_config_file(&file);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(aside.is_some(), "no config.json.bad-*: {names:?}");
    assert_eq!(aside_bytes.as_deref(), Some(&b"{"[..]));
    assert!(
        matches!(after, LoadedConfigFile::Parsed(_)),
        "new file: {after:?}"
    );
    assert_eq!(run.file_report.state, FileState::Ok);
    assert!(run.file_report.persistent_identity);
    assert!(!run.file_report.identity_declined);
}

#[test]
fn newer_version_not_touched() {
    let dir = scratch("newer");
    let file = dir.join(FILE_NAME);
    let text = r#"{"rev": 1, "config": {"version": 2}}"#;
    std::fs::write(&file, text).expect("write");
    let run = prepare(&explicit(&file), &CliOverrides::default(), false, NOW).expect("prepare");
    let after = std::fs::read_to_string(&file).expect("read");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(run.file_report.state, FileState::Invalid);
    assert_eq!(after, text, "the file changed");
    assert!(!run.file_report.persistent_identity);
    assert!(!run.file_report.identity_declined);
}

#[test]
fn disabled_touches_nothing() {
    let dir = scratch("disabled");
    let cli = CliOverrides {
        socks_port: Some(2000),
        ..CliOverrides::default()
    };
    let run = prepare(&ConfigSource::Disabled, &cli, false, NOW).expect("prepare");
    let names = names_in(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(names.is_empty(), "in the directory: {names:?}");
    assert!(run.file_report.identity_declined);
    assert!(!run.file_report.persistent_identity);
    assert_eq!(run.file_report.state, FileState::Ok);
    assert_eq!(run.file_report.path, None);
    assert!(!run.file_report.writable);
    assert_eq!(run.effective_settings.process.socks_port, 2000);
    assert_eq!(run.cli_override_paths, vec!["socks.port"]);
}

#[test]
fn absent_file_is_created_with_two_word_name() {
    let dir = scratch("absent-name");
    let file = dir.join(FILE_NAME);
    let run = prepare(&explicit(&file), &CliOverrides::default(), false, NOW).expect("prepare");
    let on_disk = parsed(&file);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(run.file_report.state, FileState::Created);
    let name = run
        .effective_settings
        .net
        .node_name
        .clone()
        .expect("node_name in Settings");
    assert_two_pool_words(&name);
    assert_eq!(
        run.file_settings.net.node_name.as_deref(),
        Some(name.as_str()),
        "the name must also be in file_settings"
    );
    assert_eq!(
        on_disk.settings.net.node_name.as_deref(),
        Some(name.as_str()),
        "the name must land in the file"
    );
}

#[cfg(unix)]
#[test]
fn uncreatable_file_still_names_node() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("uncreatable-name");
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
    let got = prepare(&explicit(&file), &CliOverrides::default(), false, NOW);
    let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    let file_exists = file.exists();
    let _ = std::fs::remove_dir_all(&dir);
    let run = got.expect("the node must start even without writing the file");
    assert_eq!(run.file_report.state, FileState::Unreadable);
    assert!(!file_exists, "the file must not have been created");
    let name = run
        .effective_settings
        .net
        .node_name
        .clone()
        .expect("node_name in Settings");
    assert_two_pool_words(&name);
}

#[cfg(unix)]
#[test]
fn readonly_dir_still_starts() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("readonly-start");
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
    let got = prepare(&explicit(&file), &CliOverrides::default(), false, NOW);
    let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    let _ = std::fs::remove_dir_all(&dir);
    let run = got.expect("the node must start even without writing");
    assert_eq!(run.file_report.state, FileState::Unreadable);
    assert!(!run.file_report.writable);
    assert!(!run.file_report.persistent_identity);
    assert!(!run.file_report.identity_declined);
    assert!(!run.file_report.overwrite_refused);
}

/// The default node name: two different pool words joined by a hyphen, without a tail.
fn assert_two_pool_words(name: &str) {
    let parts: Vec<&str> = name.split('-').collect();
    assert_eq!(parts.len(), 2, "{name}");
    assert!(crate::members::words::is_tail_word(parts[0]), "{name}");
    assert!(crate::members::words::is_tail_word(parts[1]), "{name}");
    assert_ne!(parts[0], parts[1], "{name}");
}
