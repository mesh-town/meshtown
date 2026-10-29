//! Tests of the process services: the guard thread's life, the summary sources, the dump directory.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::{configure_exit_limits, heap_dump_dir, ExitLimits, MemoryDefaults, ProcessServices};
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
async fn set_and_clear_move_the_sources() {
    let node = start_node(NodeConfig::for_loopback_test(
        NetworkKey::generate().unwrap(),
    ))
    .await
    .unwrap();
    let services = ProcessServices::start(&settings(0, 0), PathBuf::from("."));
    let sources = services.sources();
    let other = NetSlotId::parse("ab2c3d").unwrap();
    assert!(services.sources().snapshot().is_empty());
    sources.set(NetSlotId::main(), node.members.clone(), node.checks.clone());
    sources.set(other.clone(), node.members.clone(), node.checks.clone());
    assert_eq!(
        services.sources().snapshot().len(),
        2,
        "both slots are seen"
    );
    sources.clear(&other);
    assert_eq!(services.sources().snapshot().len(), 1, "one slot is gone");
    sources.clear(&NetSlotId::main());
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

mod exit_window {
    use super::*;

    #[test]
    fn small_machine_gets_the_small_window() {
        // given: a small router, the setting left at 0
        let limits = ExitLimits::fixed(0);

        // when: the node starts the exit limits
        start_limits(
            &limits,
            ProcessSettings::default(),
            MemoryDefaults::SMALL,
            64 * 1024,
        );

        // then: streams get the 128 KiB window of the small tier
        assert_eq!(limits.stream_window_bytes(), 131_072);
    }

    #[test]
    fn middle_machine_has_no_window() {
        // given: a mid-size router, the setting left at 0
        let limits = ExitLimits::fixed(0);

        // when: the node starts the exit limits
        start_limits(
            &limits,
            ProcessSettings::default(),
            MemoryDefaults::MIDDLE,
            256 * 1024,
        );

        // then: there is no window
        assert_eq!(limits.stream_window_bytes(), 0);
    }

    #[test]
    fn pc_gets_no_window() {
        // given: a large machine, and a machine whose memory is unknown, the setting left at 0
        for (defaults, ram) in [
            (MemoryDefaults::LARGE, Some(16 * 1024 * 1024)),
            (MemoryDefaults::LARGE, None),
        ] {
            let limits = ExitLimits::fixed(0);

            // when: the node starts the exit limits
            configure_exit_limits(&limits, &ProcessSettings::default(), defaults, ram, 400);

            // then: there is no window
            assert_eq!(limits.stream_window_bytes(), 0, "ram {ram:?}");
        }
    }

    #[test]
    fn explicit_window_wins_on_a_pc() {
        // given: a large machine and a setting of 256 KiB
        let limits = ExitLimits::fixed(0);
        let process = ProcessSettings {
            exit_stream_window_kib: 256,
            ..ProcessSettings::default()
        };

        // when: the node starts the exit limits
        start_limits(&limits, process, MemoryDefaults::LARGE, 16 * 1024 * 1024);

        // then: the window is the setting
        assert_eq!(limits.stream_window_bytes(), 262_144);
    }

    fn start_limits(
        limits: &ExitLimits,
        process: ProcessSettings,
        defaults: MemoryDefaults,
        ram_kib: u64,
    ) {
        configure_exit_limits(limits, &process, defaults, Some(ram_kib), defaults.warn_mib);
    }
}
