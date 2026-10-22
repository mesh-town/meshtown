//! Tests of the process services: the guard thread's life, the summary sources, the dump directory.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::{heap_dump_dir, ProcessServices};
use crate::config::config_file::{NetSlotId, ProcessSettings};
use crate::members::network_key::NetworkKey;
use crate::node::guard::GuardHandle;
use crate::node::start::start_node;
use crate::node::NodeConfig;

const DEADLINE: Duration = Duration::from_secs(3);

fn settings(warn_mib: u64, exit_mib: u64) -> ProcessSettings {
    ProcessSettings {
        node_memory_warn_mib: warn_mib,
        node_memory_exit_mib: exit_mib,
        ..ProcessSettings::default()
    }
}

fn wait_finished(h: &GuardHandle) -> bool {
    let end = Instant::now() + DEADLINE;
    while Instant::now() < end && !h.is_finished() {
        std::thread::sleep(Duration::from_millis(20));
    }
    h.is_finished()
}

#[tokio::test]
async fn start_runs_the_guard_and_stop_ends_it() {
    let services = ProcessServices::start(&settings(400, 900), PathBuf::from("."));
    let h = services.guard_handle().expect("the guard must start");
    assert!(!h.is_finished());
    services.stop();
    assert!(
        wait_finished(&h),
        "the mem-guard thread is alive after stop"
    );
}

#[tokio::test]
async fn zero_thresholds_start_no_guard() {
    let services = ProcessServices::start(&settings(0, 0), PathBuf::from("."));
    assert!(services.guard_handle().is_none());
    services.stop();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn register_and_unregister_move_the_sources() {
    let node = start_node(NodeConfig::for_loopback_test(
        NetworkKey::generate().unwrap(),
    ))
    .await
    .unwrap();
    let services = ProcessServices::start(&settings(0, 0), PathBuf::from("."));
    let other = NetSlotId::parse("ab2c3d").unwrap();
    assert!(services.sources().snapshot().is_empty());
    services.register(NetSlotId::main(), node.members.clone(), node.checks.clone());
    services.register(other.clone(), node.members.clone(), node.checks.clone());
    assert_eq!(
        services.sources().snapshot().len(),
        2,
        "both slots are seen"
    );
    services.unregister(&other);
    assert_eq!(services.sources().snapshot().len(), 1, "one slot is gone");
    services.unregister(&NetSlotId::main());
    assert!(services.sources().snapshot().is_empty());
    services.stop();
    node.shutdown().await;
}

#[test]
fn heap_dump_dir_beside_the_settings_file() {
    let dir = heap_dump_dir(Some(Path::new("/data/a/config.json")));
    assert_eq!(dir, PathBuf::from("/data/a"));
}

#[test]
fn heap_dump_dir_for_a_bare_file_name_is_its_empty_parent() {
    let dir = heap_dump_dir(Some(Path::new("config.json")));
    assert_eq!(dir, PathBuf::from(""));
}

#[test]
fn heap_dump_dir_defaults_to_current() {
    assert_eq!(heap_dump_dir(None), PathBuf::from("."));
}
