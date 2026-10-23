//! Tests of leaving a network: an added slot, the main one, a broken one, and the timing of the stop.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::*;
use crate::config::config_file::{
    self, CliOverrides, ConfigSource, LoadedConfigFile, PathRule, RunConfig, Settings,
};
use crate::node::node_ctl::NodeCtl;
use crate::node::output::{self, log_page};
use crate::web_gui::{ConfigState, HostControl, NodeStatus, SlotHandle};

const KEY: &str = "team--0123456789abcdef0123456789abcdef";

struct Fixture {
    dir: PathBuf,
    slots: Arc<HostSlots>,
    main_ctl: Arc<NodeCtl>,
}

impl Fixture {
    fn main_file(&self) -> PathBuf {
        self.dir.join(config_file::FILE_NAME)
    }

    fn left_dirs(&self, prefix: &str) -> Vec<PathBuf> {
        let Ok(read) = std::fs::read_dir(self.dir.join("nets")) else {
            return Vec::new();
        };
        let mut found: Vec<PathBuf> = read
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                name.starts_with(prefix)
            })
            .collect();
        found.sort();
        found
    }
}

fn temp_dir() -> PathBuf {
    static COUNTER: portable_atomic::AtomicU64 = portable_atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("mesh-retire-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A map whose main slot has a settings file holding a network key (or none).
fn fixture(with_key: bool, cli_key: bool) -> Fixture {
    let dir = temp_dir();
    let source = ConfigSource::File {
        path: dir.join(config_file::FILE_NAME),
        may_create: true,
        rule: PathRule::Explicit,
    };
    let run = config_file::prepare(&source, &CliOverrides::default(), true, 0).unwrap();
    let mut state = ConfigState::from_run(&run);
    if with_key {
        let mut s = state.file_settings.clone();
        s.net.network_key = Some(KEY.to_string());
        let path = dir.join(config_file::FILE_NAME);
        let rev =
            config_file::save_config_file(&path, Some(state.file_report.rev), 1, &state.secret, &s)
                .unwrap();
        state.file_settings = s;
        state.file_report.rev = rev;
        state.current_network_key = Some(KEY.to_string());
    }
    if cli_key {
        state.cli_override_paths = vec!["network.key"];
    }
    let main_ctl = NodeCtl::new(&Settings::default());
    let slots = HostSlots::with_main(main_ctl.clone(), state);
    Fixture {
        dir,
        slots,
        main_ctl,
    }
}

fn placeholder_run() -> RunConfig {
    config_file::prepare(&ConfigSource::Disabled, &CliOverrides::default(), false, 0).unwrap()
}

fn id_of(text: &str) -> NetSlotId {
    NetSlotId::parse(text).unwrap()
}

/// Adds a slot whose cycle ends when it is told to stop; its folder holds a settings file.
fn add_running(fx: &Fixture, id: &str) -> Arc<NodeCtl> {
    let ctl = NodeCtl::new(&Settings::default());
    let handle = SlotHandle {
        id: id_of(id),
        node: ctl.clone(),
        config: Arc::new(Mutex::new(ConfigState::from_run(&placeholder_run()))),
    };
    let waiting = ctl.clone();
    fx.slots
        .push_slot(handle, ctl.clone(), async move {
            waiting.next_command().await;
            Ok(())
        })
        .unwrap();
    make_folder(&fx.main_file(), id);
    ctl
}

fn make_folder(main: &Path, id: &str) {
    let file = config_file::net_slot_path(main, &id_of(id)).unwrap();
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, b"{}").unwrap();
}

async fn finish(slots: &HostSlots) {
    for task in slots.take_retiring() {
        task.await.unwrap();
    }
}

/// Plays the node cycle of the main slot: takes the leave command and waits for a key.
fn drive_leave(ctl: &Arc<NodeCtl>) -> tokio::task::JoinHandle<()> {
    let ctl = ctl.clone();
    output::spawn(async move {
        ctl.next_command().await;
        ctl.detach(NodeStatus::Starting);
        ctl.mark_no_network();
    })
}

fn log_has(text: &str) -> bool {
    log_page(0, 5000)
        .lines
        .iter()
        .any(|l| l.text.contains(text))
}

fn file_key(dir: &Path) -> Option<String> {
    match config_file::load_config_file(&dir.join(config_file::FILE_NAME)) {
        LoadedConfigFile::Parsed(p) => p.settings.net.network_key,
        _ => panic!("the settings file can't be read"),
    }
}

#[tokio::test(start_paused = true)]
async fn an_added_slot_leaves_the_map_and_its_folder_is_set_aside() {
    let fx = fixture(false, false);
    add_running(&fx, "ab2c3d");
    fx.slots.retire(&id_of("ab2c3d"), false).unwrap();
    assert!(fx.slots.slot(&id_of("ab2c3d")).is_none());
    finish(&fx.slots).await;
    assert_eq!(fx.left_dirs("left-ab2c3d-").len(), 1);
    assert!(!fx.dir.join("nets").join("ab2c3d").exists());
}

#[tokio::test(start_paused = true)]
async fn the_stop_waits_for_the_grace_only_when_announced() {
    let fx = fixture(false, false);
    let quick = add_running(&fx, "ab2c3d");
    let slow = add_running(&fx, "ab2c4d");
    fx.slots.retire(&id_of("ab2c3d"), false).unwrap();
    fx.slots.retire(&id_of("ab2c4d"), true).unwrap();
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert!(quick.stop_requested());
    assert!(!slow.stop_requested());
    tokio::time::sleep(LEAVE_GRACE - Duration::from_millis(100)).await;
    assert!(!slow.stop_requested());
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(slow.stop_requested());
    finish(&fx.slots).await;
}

#[tokio::test(start_paused = true)]
async fn a_slot_that_never_got_a_key_has_no_folder_to_move() {
    let fx = fixture(false, false);
    add_running(&fx, "ab2c3d");
    std::fs::remove_dir_all(fx.dir.join("nets")).unwrap();
    fx.slots.retire(&id_of("ab2c3d"), false).unwrap();
    finish(&fx.slots).await;
    assert!(fx.left_dirs("left-").is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_node_that_does_not_stop_keeps_its_folder() {
    let fx = fixture(false, false);
    let ctl = NodeCtl::new(&Settings::default());
    let handle = SlotHandle {
        id: id_of("ab2c3d"),
        node: ctl.clone(),
        config: Arc::new(Mutex::new(ConfigState::from_run(&placeholder_run()))),
    };
    fx.slots
        .push_slot(handle, ctl, std::future::pending())
        .unwrap();
    make_folder(&fx.main_file(), "ab2c3d");
    fx.slots.retire(&id_of("ab2c3d"), false).unwrap();
    finish(&fx.slots).await;
    assert!(fx.dir.join("nets").join("ab2c3d").is_dir());
    assert!(log_has("the node did not stop in time"));
}

#[tokio::test(start_paused = true)]
async fn the_retirement_task_is_awaited_by_the_host_and_none_starts_after_the_stop() {
    let fx = fixture(false, false);
    add_running(&fx, "ab2c3d");
    add_running(&fx, "ab2c4d");
    fx.slots.retire(&id_of("ab2c3d"), true).unwrap();
    fx.slots.stop_all();
    let refused = fx.slots.retire(&id_of("ab2c4d"), false);
    assert!(matches!(refused, Err(HostError::Busy(_))), "{refused:?}");
    assert!(fx.slots.slot(&id_of("ab2c4d")).is_some());
    let tasks = fx.slots.take_retiring();
    assert_eq!(tasks.len(), 1);
    for task in tasks {
        task.await.unwrap();
    }
    assert_eq!(fx.left_dirs("left-ab2c3d-").len(), 1);
}

#[tokio::test(start_paused = true)]
async fn an_unknown_slot_is_unknown() {
    let fx = fixture(false, false);
    let got = fx.slots.retire(&id_of("ab2c3d"), false);
    assert!(matches!(got, Err(HostError::UnknownNet)), "{got:?}");
}

#[tokio::test(start_paused = true)]
async fn the_main_slot_without_a_key_has_nothing_to_leave() {
    let fx = fixture(false, false);
    let before = std::fs::read(fx.main_file()).unwrap();
    assert!(fx.slots.retire(&NetSlotId::main(), true).is_ok());
    assert!(fx.slots.take_retiring().is_empty());
    assert_eq!(std::fs::read(fx.main_file()).unwrap(), before);
    assert!(fx.left_dirs("left-").is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_key_from_a_flag_cannot_be_left() {
    let fx = fixture(true, true);
    let got = fx.slots.retire(&NetSlotId::main(), false);
    assert!(matches!(got, Err(HostError::Busy(_))), "{got:?}");
    assert!(fx.slots.take_retiring().is_empty());
}

#[tokio::test(start_paused = true)]
async fn the_main_network_is_left_and_its_files_are_set_aside() {
    let fx = fixture(true, false);
    let old_file = std::fs::read(fx.main_file()).unwrap();
    let main = fx.slots.main_slot();
    let old_secret = main.config.lock().unwrap().secret.to_bytes();
    let driver = drive_leave(&fx.main_ctl);
    fx.slots.retire(&NetSlotId::main(), false).unwrap();
    finish(&fx.slots).await;
    driver.await.unwrap();
    assert!(fx.main_ctl.is_no_network());
    assert_eq!(file_key(&fx.dir), None);
    let state = main.config.lock().unwrap();
    assert_ne!(state.secret.to_bytes(), old_secret);
    assert_eq!(state.current_network_key, None);
    let aside = fx.left_dirs("left-main-");
    assert_eq!(aside.len(), 1);
    let copy = std::fs::read(aside[0].join(config_file::FILE_NAME)).unwrap();
    assert_eq!(copy, old_file);
}

#[tokio::test(start_paused = true)]
async fn a_failed_set_aside_keeps_the_file_and_changes_the_memory() {
    let fx = fixture(true, false);
    std::fs::write(fx.dir.join("nets"), b"not a folder").unwrap();
    let old_file = std::fs::read(fx.main_file()).unwrap();
    let driver = drive_leave(&fx.main_ctl);
    fx.slots.retire(&NetSlotId::main(), false).unwrap();
    finish(&fx.slots).await;
    driver.await.unwrap();
    assert_eq!(std::fs::read(fx.main_file()).unwrap(), old_file);
    let main = fx.slots.main_slot();
    assert_eq!(main.config.lock().unwrap().current_network_key, None);
    assert!(log_has("network files not moved"));
}

#[tokio::test(start_paused = true)]
async fn a_run_without_a_file_only_drops_the_key_in_memory() {
    let mut state = ConfigState::from_run(&placeholder_run());
    state.current_network_key = Some(KEY.to_string());
    let main_ctl = NodeCtl::new(&Settings::default());
    let slots = HostSlots::with_main(main_ctl.clone(), state);
    let driver = drive_leave(&main_ctl);
    slots.retire(&NetSlotId::main(), false).unwrap();
    finish(&slots).await;
    driver.await.unwrap();
    let main = slots.main_slot();
    assert_eq!(main.config.lock().unwrap().current_network_key, None);
}

#[tokio::test(start_paused = true)]
async fn a_node_that_does_not_leave_in_time_leaves_the_files_alone() {
    let fx = fixture(true, false);
    let old_file = std::fs::read(fx.main_file()).unwrap();
    fx.slots.retire(&NetSlotId::main(), false).unwrap();
    finish(&fx.slots).await;
    assert_eq!(std::fs::read(fx.main_file()).unwrap(), old_file);
    assert!(fx.left_dirs("left-").is_empty());
    let main = fx.slots.main_slot();
    assert_eq!(
        main.config.lock().unwrap().current_network_key.as_deref(),
        Some(KEY)
    );
}

#[tokio::test(start_paused = true)]
async fn leaving_a_node_that_is_stopping_changes_no_file() {
    let fx = fixture(true, false);
    let old_file = std::fs::read(fx.main_file()).unwrap();
    fx.main_ctl.stop_for_leave();
    fx.slots.retire(&NetSlotId::main(), false).unwrap();
    finish(&fx.slots).await;
    assert_eq!(std::fs::read(fx.main_file()).unwrap(), old_file);
    assert!(log_has("couldn't leave the network"));
}

#[tokio::test(start_paused = true)]
async fn a_broken_slot_is_forgotten_and_its_folder_set_aside() {
    let fx = fixture(false, false);
    make_folder(&fx.main_file(), "ab2c3d");
    fx.slots.mark_broken(id_of("ab2c3d"));
    fx.slots.retire(&id_of("ab2c3d"), false).unwrap();
    assert!(fx.slots.broken().is_empty());
    finish(&fx.slots).await;
    assert_eq!(fx.left_dirs("left-ab2c3d-").len(), 1);
    let again = fx.slots.retire(&id_of("ab2c3d"), false);
    assert!(matches!(again, Err(HostError::UnknownNet)), "{again:?}");
}
