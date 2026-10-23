use std::cell::Cell;

use super::new_net_slot;
use crate::config::config_file::test_support::{scratch, test_secret};
use crate::config::config_file::{
    prepare, save_config_file, CliOverrides, ConfigSource, NetSettings, PathRule, RunConfig,
    Settings, FILE_NAME,
};

const NOW: u64 = 1_790_278_867;

fn main_run() -> (std::path::PathBuf, RunConfig) {
    let root = scratch("slot-new");
    let file = root.join(FILE_NAME);
    save_config_file(&file, None, 1, &test_secret(), &Settings::default()).expect("main file");
    let source = ConfigSource::File {
        path: file.clone(),
        may_create: true,
        rule: PathRule::Explicit,
    };
    let run = prepare(&source, &CliOverrides::default(), false, NOW).expect("main run");
    (file, run)
}

#[test]
fn a_free_id_is_returned_with_its_unwritten_run() {
    let (file, main) = main_run();
    let (id, run) =
        new_net_slot(&main, &file, &NetSettings::default(), &|_| false, NOW).expect("slot");
    let path = run.file.as_ref().expect("slot file").path().to_path_buf();
    assert!(path.to_string_lossy().contains(&id.to_string()));
    assert!(!path.exists());
}

#[test]
fn a_taken_id_is_skipped() {
    let (file, main) = main_run();
    let calls = Cell::new(0);
    let taken = |_: &_| {
        calls.set(calls.get() + 1);
        calls.get() == 1
    };
    new_net_slot(&main, &file, &NetSettings::default(), &taken, NOW).expect("slot");
    assert_eq!(calls.get(), 2);
}

#[test]
fn running_out_of_attempts_is_an_error() {
    let (file, main) = main_run();
    let Err(err) = new_net_slot(&main, &file, &NetSettings::default(), &|_| true, NOW) else {
        panic!("no free id expected");
    };
    assert_eq!(err.to_string(), "couldn't find a free network id");
}
