//! Scenarios for the `config.json.bad-*` copy before an overwriting start, and write refusals
//! (readonly, a taken copy name). Moved out of `run_tests.rs` (code layout) —
//! test bodies and names are unchanged.

use std::path::Path;

use super::*;
use crate::config::config_file::overrides::CliOverrides;
use crate::config::config_file::path::{ConfigSource, PathRule};
use crate::config::config_file::save::save_config_file;
use crate::config::config_file::settings::Settings;
use crate::config::config_file::test_support::{
    clear_readonly, names_in, parsed, scratch, secret_of, test_secret, zero_secret_b64,
};
use crate::config::config_file::time::stamp_utc;
use crate::config::config_file::FILE_NAME;
use crate::members::network_key::managed_key_with_label;
use crate::transport::net::NodeSecret;

const NOW: u64 = 1_790_278_867;

fn explicit(path: &Path) -> ConfigSource {
    ConfigSource::File {
        path: path.to_path_buf(),
        may_create: true,
        rule: PathRule::Explicit,
    }
}

/// Returns the `config.json.bad-*` copy names among `names`.
fn bad_copies(names: &[String]) -> Vec<String> {
    names
        .iter()
        .filter(|n| n.starts_with("config.json.bad-"))
        .cloned()
        .collect()
}

/// The permission bits of the file at `path`.
#[cfg(unix)]
fn file_mode(path: &std::path::Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).expect("copy").permissions().mode() & 0o777
}

/// The node secret is corrupted: before the start write that replaces it, the original file (with
/// the network key) is copied to `config.json.bad-<stamp>`.
#[test]
fn bad_secret_startup_write_keeps_copy() {
    let dir = scratch("copy-bad-secret");
    let file = dir.join(FILE_NAME);
    let key = managed_key_with_label(
        None,
        &[7u8; 32],
        &NodeSecret::from_bytes(&[1u8; 32]).peer_id(),
    );
    let text = format!(
        r#"{{"rev": 3, "identity": {{"secret": "broken"}},
        "config": {{"version": 1, "network": {{"key": "{key}"}}}}}}"#
    );
    std::fs::write(&file, &text).expect("writing the test file");
    let run = prepare(&explicit(&file), &CliOverrides::default(), false, NOW).expect("prepare");
    let names = names_in(&dir);
    let copies = bad_copies(&names);
    let copy_bytes = copies
        .first()
        .map(|n| std::fs::read(dir.join(n)).expect("reading the copy"));
    #[cfg(unix)]
    let copy_mode = copies.first().map(|n| file_mode(&dir.join(n)));
    let after = parsed(&file);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(copies.len(), 1, "files in the directory: {names:?}");
    assert_eq!(names.len(), 2, "files in the directory: {names:?}");
    assert_eq!(copies[0], format!("{FILE_NAME}.bad-{}", stamp_utc(NOW)));
    assert_eq!(
        copy_bytes.as_deref(),
        Some(text.as_bytes()),
        "the copy differs from the original file"
    );
    #[cfg(unix)]
    assert_eq!(copy_mode, Some(0o600), "copy mode {copy_mode:?}");
    assert_eq!(
        secret_of(&after),
        run.secret.to_bytes(),
        "the new file has the wrong secret"
    );
    assert_eq!(
        after.settings.net.network_key.as_deref(),
        Some(key.as_str())
    );
    assert_eq!(after.rev, 4);
    assert_key_parsed(&run);
}

/// The start report shows a persistent identity and no error on the network key.
fn assert_key_parsed(run: &RunConfig) {
    assert!(
        !run.file_report
            .field_errors
            .iter()
            .any(|e| e.field_path == "network.key"),
        "the network key did not parse: {:?}",
        run.file_report.field_errors
    );
    assert!(run.file_report.persistent_identity);
    assert!(!run.file_report.overwrite_refused);
}

/// A field is out of bounds and there is no secret in the file: the start write replaces the field with its default,
/// so a copy comes first.
#[test]
fn field_error_startup_write_keeps_copy() {
    let dir = scratch("copy-field-error");
    let file = dir.join(FILE_NAME);
    let text = r#"{"rev": 2, "config": {"version": 1, "socks": {"port": 80}}}"#;
    std::fs::write(&file, text).expect("writing the test file");
    let run = prepare(&explicit(&file), &CliOverrides::default(), false, NOW).expect("prepare");
    let names = names_in(&dir);
    let copies = bad_copies(&names);
    let copy_bytes = copies
        .first()
        .map(|n| std::fs::read(dir.join(n)).expect("reading the copy"));
    let after = parsed(&file);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(copies.len(), 1, "files in the directory: {names:?}");
    assert_eq!(
        copy_bytes.as_deref(),
        Some(text.as_bytes()),
        "the copy differs from the original file"
    );
    assert_eq!(
        after.settings.process.socks_port, 1080,
        "the corrupted field was not replaced by its default"
    );
    assert_eq!(after.rev, 3);
    assert!(run.file_report.persistent_identity);
    assert!(!run.file_report.overwrite_refused);
}

/// The secret is fine but there is a corrupted field, and `--save-config` will rewrite the file: a copy is needed.
#[test]
fn save_config_with_field_error_keeps_copy() {
    let dir = scratch("copy-save-config");
    let file = dir.join(FILE_NAME);
    let text = format!(
        r#"{{"rev": 2, "identity": {{"secret": "{}"}},
        "config": {{"version": 1, "socks": {{"port": 80}}}}}}"#,
        zero_secret_b64()
    );
    std::fs::write(&file, &text).expect("writing the test file");
    let run = prepare(&explicit(&file), &CliOverrides::default(), true, NOW).expect("prepare");
    let names = names_in(&dir);
    let copies = bad_copies(&names);
    let copy_bytes = copies
        .first()
        .map(|n| std::fs::read(dir.join(n)).expect("reading the copy"));
    let after = parsed(&file);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(copies.len(), 1, "files in the directory: {names:?}");
    assert_eq!(
        copy_bytes.as_deref(),
        Some(text.as_bytes()),
        "the copy differs from the original file"
    );
    assert_eq!(after.rev, 3);
    assert_eq!(run.file_report.rev, 3);
    assert_eq!(after.settings.process.socks_port, 1080);
    assert_eq!(
        secret_of(&after),
        [0u8; 32],
        "the secret from the file must survive"
    );
    assert_eq!(run.secret.to_bytes(), [0u8; 32]);
    assert!(!run.file_report.overwrite_refused);
}

/// A clean file triggers no copy: neither when the start leaves it alone, nor when it adds a
/// missing secret, nor with an unknown field (those vanish as intended).
#[test]
fn clean_file_makes_no_copy() {
    // A clean file with a real secret, no `--save-config`: the file is unchanged.
    let dir = scratch("no-copy-clean");
    let file = dir.join(FILE_NAME);
    save_config_file(&file, None, 1, &test_secret(), &Settings::default()).expect("save");
    let before = std::fs::read(&file).expect("reading before");
    let first = prepare(&explicit(&file), &CliOverrides::default(), false, NOW).expect("prepare");
    let after = std::fs::read(&file).expect("reading after");
    let names = names_in(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(names, vec![FILE_NAME.to_string()], "extra files");
    assert_eq!(after, before, "the clean file changed");
    assert!(!first.file_report.overwrite_refused);

    // No secret, no errors, an unknown field: the secret is added, no copy.
    let dir = scratch("no-copy-missing-secret");
    let file = dir.join(FILE_NAME);
    std::fs::write(&file, r#"{"rev": 1, "config": {"version": 1, "zzz": 1}}"#)
        .expect("writing the test file");
    let run = prepare(&explicit(&file), &CliOverrides::default(), false, NOW).expect("prepare");
    let names = names_in(&dir);
    let after = parsed(&file);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(names, vec![FILE_NAME.to_string()], "extra files");
    assert_eq!(after.rev, 2, "the secret should have been added");
    assert!(run.file_report.persistent_identity);
    assert!(!run.file_report.overwrite_refused);
}

/// The copy could not be made: the original file stays as it was, the start goes on without a write. The refusal
/// is caused by a taken copy name (a directory with the same name; the stamp comes from `NOW`), not by
/// a permissions trick.
#[test]
fn copy_failure_keeps_original() {
    let dir = scratch("copy-failure");
    let file = dir.join(FILE_NAME);
    let text = format!(
        r#"{{"rev": 3, "identity": {{"secret": "broken"}},
        "config": {{"version": 1, "network": {{"key": "{}"}}}}}}"#,
        managed_key_with_label(
            None,
            &[7u8; 32],
            &NodeSecret::from_bytes(&[1u8; 32]).peer_id()
        )
    );
    std::fs::write(&file, &text).expect("writing the test file");
    let taken = format!("{FILE_NAME}.bad-{}", stamp_utc(NOW));
    std::fs::create_dir(dir.join(&taken)).expect("a directory in place of the copy");
    let run = prepare(&explicit(&file), &CliOverrides::default(), false, NOW).expect("prepare");
    let after = std::fs::read(&file).expect("reading after");
    let mut names = names_in(&dir);
    names.sort();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(
        after,
        text.as_bytes(),
        "the original file was overwritten without a copy"
    );
    let mut want = vec![FILE_NAME.to_string(), taken];
    want.sort();
    assert_eq!(names, want, "extra files appeared in the directory");
    assert!(!run.file_report.persistent_identity);
    assert!(!run.file_report.writable);
    assert!(run.file_report.overwrite_refused);
    assert_eq!(run.file_report.rev, 3, "the revision must not grow");
    let message = run.file_report.message.as_deref().unwrap_or("");
    assert!(
        message.contains("original file not overwritten"),
        "message: {message}"
    );
}

/// A file without `identity.secret`, marked readonly: `prepare` does not write it,
/// the identity does not become persistent, the file is unchanged byte for byte.
#[test]
fn missing_secret_readonly_file_preserved() {
    let dir = scratch("missing-secret-readonly");
    let file = dir.join(FILE_NAME);
    let text = r#"{"rev": 4, "config": {"version": 1}}"#;
    std::fs::write(&file, text).expect("writing the test file");
    let mut perms = std::fs::metadata(&file).expect("metadata").permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&file, perms).expect("readonly");
    let run = prepare(&explicit(&file), &CliOverrides::default(), false, NOW);
    let after = std::fs::read_to_string(&file).expect("reading after");
    let names = names_in(&dir);
    clear_readonly(&file);
    let _ = std::fs::remove_dir_all(&dir);
    let run = run.expect("prepare");
    assert_eq!(after, text, "the file changed");
    assert_eq!(names, vec![FILE_NAME.to_string()], "extra files: {names:?}");
    assert!(run.file_report.overwrite_refused);
    assert!(!run.file_report.persistent_identity);
    let message = run.file_report.message.as_deref().unwrap_or("");
    assert!(message.contains("read-only"), "message: {message}");
}

/// `--save-config` on a valid readonly file: the same refusal branch, the file is unchanged,
/// no `.bad-*` copies appear.
#[test]
fn save_config_readonly_file_refused() {
    let dir = scratch("save-config-readonly");
    let file = dir.join(FILE_NAME);
    save_config_file(&file, None, 1, &test_secret(), &Settings::default())
        .expect("saving the original file");
    let before = std::fs::read(&file).expect("reading before");
    let mut perms = std::fs::metadata(&file).expect("metadata").permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&file, perms).expect("readonly");
    let run = prepare(&explicit(&file), &CliOverrides::default(), true, NOW);
    let after = std::fs::read(&file).expect("reading after");
    let names = names_in(&dir);
    clear_readonly(&file);
    let _ = std::fs::remove_dir_all(&dir);
    let run = run.expect("prepare");
    assert_eq!(after, before, "the file changed");
    assert_eq!(
        names,
        vec![FILE_NAME.to_string()],
        "extra files (including .bad-*): {names:?}"
    );
    assert!(run.file_report.overwrite_refused);
    let message = run.file_report.message.as_deref().unwrap_or("");
    assert!(message.contains("read-only"), "message: {message}");
}
