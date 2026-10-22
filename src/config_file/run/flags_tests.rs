//! Scenarios for CLI flags on top of the file: a write with `--save-config` without a file, the effect
//! of a flag without writing to the file, no secret or network key in error texts.
//! Moved out of `run_tests.rs` (code layout) — test bodies and names are unchanged.

use std::path::Path;

use super::*;
use crate::config_file::overrides::CliOverrides;
use crate::config_file::parse::{load_config_file, LoadedConfigFile, SecretRead};
use crate::config_file::path::{ConfigSource, PathRule};
use crate::config_file::report::FileState;
use crate::config_file::save::save_config_file;
use crate::config_file::settings::Settings;
use crate::config_file::test_support::{parsed, scratch, secret_of, test_secret};
use crate::config_file::FILE_NAME;

const NOW: u64 = 1_790_278_867;

fn explicit(path: &Path) -> ConfigSource {
    ConfigSource::File {
        path: path.to_path_buf(),
        may_create: true,
        rule: PathRule::Explicit,
    }
}

/// `--save-config` with a missing file: the file is created, and it already holds the given flags.
#[test]
fn save_config_without_file_writes_flags() {
    let dir = scratch("save-config-absent");
    let file = dir.join(FILE_NAME);
    let cli = CliOverrides {
        socks_port: Some(2000),
        ..CliOverrides::default()
    };
    let run = prepare(&explicit(&file), &cli, true, NOW).expect("prepare");
    let exists = file.exists();
    let on_disk = match load_config_file(&file) {
        LoadedConfigFile::Parsed(p) => Some(p),
        _ => None,
    };
    let _ = std::fs::remove_dir_all(&dir);
    assert!(exists, "the file was not created");
    let on_disk = on_disk.expect("the file did not parse");
    assert_eq!(run.file_report.state, FileState::Created);
    assert_eq!(run.file_report.rev, 1);
    assert_eq!(on_disk.rev, 1);
    assert_eq!(on_disk.settings.process.socks_port, 2000, "the flag did not land in the file");
    assert_eq!(run.file_settings.process.socks_port, 2000);
    assert_eq!(run.effective_settings.process.socks_port, 2000);
    assert!(run.cli_override_paths.is_empty(), "list: {:?}", run.cli_override_paths);
    assert_eq!(secret_of(&on_disk), run.secret.to_bytes());
}

/// Without `--save-config` a flag affects the start, but does not get into the file and the file is not touched.
#[test]
fn flags_not_written_without_save_config() {
    let dir = scratch("flags-not-written");
    let file = dir.join(FILE_NAME);
    // The file holds a real secret, so the start must not add anything.
    save_config_file(&file, None, 1, &test_secret(), &Settings::default()).expect("save");
    let before = std::fs::read(&file).expect("reading before");
    let cli = CliOverrides {
        socks_port: Some(2000),
        ..CliOverrides::default()
    };
    let run = prepare(&explicit(&file), &cli, false, NOW).expect("prepare");
    let after = std::fs::read(&file).expect("reading after");
    let rev_on_disk = parsed(&file).rev;
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(run.effective_settings.process.socks_port, 2000);
    assert_eq!(run.file_settings.process.socks_port, 1080);
    assert_eq!(run.cli_override_paths, vec!["socks.port"]);
    assert_eq!(after, before, "the file on disk changed");
    assert_eq!(rev_on_disk, 1);
    assert_eq!(run.file_report.rev, 1);
}

/// Neither the network key nor the node secret gets into error texts or report `Debug`. Reading
/// the file accepts `network.key` as a non-empty string, and `prepare` parses the key,
/// so the message about the key is checked there.
#[test]
fn secrets_not_in_messages() {
    const NETKEY: &str = "MARKER-NETKEY-1234567890";
    const SECRET: &str = "MARKER-SECRET-1234567890";
    fn clean(what: &str, text: &str) {
        assert!(!text.contains("MARKER-NETKEY"), "{what}: the network key is in the text: {text}");
        assert!(!text.contains("MARKER-SECRET"), "{what}: the node secret is in the text: {text}");
    }
    let dir = scratch("no-leak");
    let file = dir.join(FILE_NAME);
    let text = format!(
        r#"{{"rev": 1, "identity": {{"secret": "{SECRET}"}},
        "config": {{"version": 1, "network": {{"key": "{NETKEY}"}}}}}}"#
    );
    std::fs::write(&file, &text).expect("writing the test file");

    let loaded = load_config_file(&file);
    clean("Debug Loaded", &format!("{loaded:?}"));
    let LoadedConfigFile::Parsed(p) = loaded else {
        panic!("expected Parsed");
    };
    assert!(matches!(&p.secret, SecretRead::Bad(_)), "{:?}", p.secret);
    clean("Debug Parsed", &format!("{p:?}"));
    for fe in &p.field_errors {
        clean(&format!("Parsed.field_errors[{}]", fe.field_path), &fe.message);
    }

    let run = prepare(&explicit(&file), &CliOverrides::default(), false, NOW).expect("prepare");
    let _ = std::fs::remove_dir_all(&dir);
    let paths: Vec<&str> = run.file_report.field_errors.iter().map(|e| e.field_path.as_str()).collect();
    assert!(paths.contains(&"network.key"), "field_errors: {paths:?}");
    assert!(paths.contains(&"identity.secret"), "field_errors: {paths:?}");
    for fe in &run.file_report.field_errors {
        clean(&format!("report.field_errors[{}]", fe.field_path), &fe.message);
    }
    clean("Debug FileReport", &format!("{:?}", run.file_report));
    clean("Debug Settings", &format!("{:?}", run.effective_settings));
    clean("Debug file Settings", &format!("{:?}", run.file_settings));

    let cli = CliOverrides {
        network_key: Some(NETKEY.to_string()),
        ..CliOverrides::default()
    };
    clean("Debug CliOverrides", &format!("{cli:?}"));
}
