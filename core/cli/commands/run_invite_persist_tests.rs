use super::*;
use crate::config::config_file::{CliOverrides, ConfigSource, PathRule};
use std::path::PathBuf;

const KEY: &str = "mesh1-key-for-the-test";

/// A directory for the settings file; removed in `Drop`.
struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        let dir =
            std::env::temp_dir().join(format!("run_invite_tests-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Dir(dir)
    }

    fn file(&self) -> PathBuf {
        self.0.join("config.json")
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A run with a settings file created in `dir`.
fn run_with_file(dir: &Dir) -> RunConfig {
    let source = ConfigSource::File {
        path: dir.file(),
        may_create: true,
        rule: PathRule::Explicit,
    };
    config_file::prepare(&source, &CliOverrides::default(), true, 0).expect("prepare")
}

fn saved_key(path: &Path) -> Option<String> {
    match config_file::load_config_file(path) {
        LoadedConfigFile::Parsed(p) => p.settings.net.network_key,
        _ => panic!("the settings file must parse"),
    }
}

#[test]
fn writes_the_key_and_keeps_the_rest_of_the_file() {
    // given
    let dir = Dir::new("writes");
    let mut run = run_with_file(&dir);
    let rev_before = run.file_report.rev;
    let mut expected = run.file_settings.clone();
    expected.net.network_key = Some(KEY.to_string());

    // when
    persist_key(&mut run, KEY, 10).unwrap();

    // then
    let LoadedConfigFile::Parsed(p) = config_file::load_config_file(&dir.file()) else {
        panic!("the settings file must parse");
    };
    assert_eq!(p.settings.net.network_key.as_deref(), Some(KEY));
    assert!(
        p.settings == expected,
        "the file must not change apart from network.key"
    );
    assert!(
        run.file_settings == expected,
        "file_settings must match the file"
    );
    assert!(run.file_report.rev > rev_before, "the revision must grow");
}

#[test]
fn without_a_file_path_writes_nothing_and_does_not_fail() {
    // given: a run without a file (`--no-config`)
    let dir = Dir::new("nopath");
    let mut run = run_with_file(&dir);
    let before = std::fs::read(dir.file()).unwrap();
    run.file_report.path = None;

    // when
    let out = persist_key(&mut run, KEY, 10);

    // then
    assert!(out.is_ok(), "the run goes on");
    assert_eq!(
        std::fs::read(dir.file()).unwrap(),
        before,
        "the file is untouched"
    );
    assert_eq!(run.file_settings.net.network_key, None);
}

#[test]
fn unreadable_file_is_a_warning_not_a_start_failure() {
    // given: the file holds garbage and cannot be parsed
    let dir = Dir::new("broken");
    let mut run = run_with_file(&dir);
    std::fs::write(dir.file(), b"{ not json").unwrap();

    // when
    let out = persist_key(&mut run, KEY, 10);

    // then
    assert!(out.is_ok(), "the run goes on");
    assert_eq!(
        std::fs::read(dir.file()).unwrap(),
        b"{ not json",
        "the file is untouched"
    );
    assert_eq!(run.file_settings.net.network_key, None);
}

#[test]
fn refused_overwrite_leaves_the_file_alone() {
    // given
    let dir = Dir::new("refused");
    let mut run = run_with_file(&dir);
    run.file_report.overwrite_refused = true;

    // when
    let err = write_key(&run, &dir.file(), KEY, 10).err();

    // then
    assert!(err.is_some(), "the write must be rejected");
    assert_eq!(saved_key(&dir.file()), None);
}
