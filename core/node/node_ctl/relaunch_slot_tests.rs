//! `Relaunch` over the file of an added network slot: reload reads the slot file and keeps the process block.

use super::*;
use crate::config::config_file::{
    save_net_file, ConfigSource, NetFile, NetSettings, ProcessSettings, RunConfig, SettingsFile,
};

const SLOT_INTERVAL: u64 = 77;

#[test]
fn reload_takes_the_net_block_from_the_slot_file_and_keeps_prev_process() {
    // given: a slot file with its own probe interval, and a previous state with a changed process block
    let fx = setup(SLOT_INTERVAL);
    let prev = Settings {
        process: ProcessSettings {
            socks_port: 4321,
            ..ProcessSettings::default()
        },
        ..Settings::default()
    };
    // when: the slot reloads under a new key
    let (s, _) = fx
        .relaunch
        .reload(&crate::node::host::test_keys::managed_text(2u8), &prev)
        .expect("reload of a slot");
    // then: the network block is the file's, the process block is the previous one
    assert_eq!(
        s.net.probe_interval_sec, SLOT_INTERVAL,
        "net block from the slot file"
    );
    assert_eq!(s.process.socks_port, 4321, "process block from prev");
}

#[test]
fn reload_sees_a_new_value_written_to_the_slot_file() {
    // given: a slot file that is rewritten after the slot was set up
    let fx = setup(SLOT_INTERVAL);
    fx.rewrite_interval(SLOT_INTERVAL + 1);
    // when: the slot reloads
    let (s, _) = fx
        .relaunch
        .reload(
            &crate::node::host::test_keys::managed_text(2u8),
            &Settings::default(),
        )
        .expect("reload of a slot");
    // then: the new value is read
    assert_eq!(s.net.probe_interval_sec, SLOT_INTERVAL + 1);
}

#[test]
fn caches_are_placed_beside_the_slot_file() {
    // given: a slot whose file is in its own folder
    let fx = setup(SLOT_INTERVAL);
    // when: the node config is built
    let cfg = fx.relaunch.node_config(
        &Settings::default(),
        crate::node::host::test_keys::key_input(2),
    );
    // then: the caches lie in the slot folder, not beside the main file
    let slot_dir = fx.path.parent().unwrap();
    assert_eq!(
        cfg.peers_cache_path,
        Some(slot_dir.join("peers-cache.json"))
    );
}

#[test]
fn dht_table_is_placed_beside_the_slot_file() {
    // given: two slots in their own folders under one parent
    let fx = setup(SLOT_INTERVAL);
    let second_file = fx.dir.join("nets").join("cd4e5f").join("config.json");
    let second = slot_relaunch(&second_file);
    // when: the node configs are built
    let first_cfg = fx.relaunch.node_config(
        &Settings::default(),
        crate::node::host::test_keys::key_input(2),
    );
    let second_cfg = second.node_config(
        &Settings::default(),
        crate::node::host::test_keys::key_input(2),
    );
    // then: each slot has its own table file in its own folder
    let first_dir = fx.path.parent().unwrap();
    assert_eq!(
        first_cfg.dht_table_path,
        Some(first_dir.join("dht-nodes.txt"))
    );
    assert_eq!(
        second_cfg.dht_table_path,
        Some(second_file.parent().unwrap().join("dht-nodes.txt"))
    );
    assert_ne!(
        first_cfg.dht_table_path, second_cfg.dht_table_path,
        "slots must not share a table file"
    );
}

mod flags_of_the_process {
    use super::*;

    #[test]
    fn slot_relaunch_keeps_the_net_flag_but_not_the_key() {
        // given: a process started with --no-exit and --key
        let main = flagged_relaunch(Some(false), None);
        // when: the relaunch of an added slot is derived from it
        let slot = main.for_net_slot(&bare_run());
        // then: the net flag is inherited, the key is not
        assert_eq!(slot.cli.exit_enabled, Some(false));
        assert_eq!(slot.cli.network_key, None, "the key belongs to one network");
    }

    #[test]
    fn net_flags_go_to_the_net_block() {
        // given: --no-exit is a flag of the process, the settings say otherwise
        let main = flagged_relaunch(Some(false), None);
        let mut s = Settings::default();
        assert!(s.net.exit_enabled);
        // when: the flags are laid over the settings of a slot
        main.apply_net_flags(&mut s);
        // then: the exit setting comes from the flag
        assert!(!s.net.exit_enabled);
    }

    #[test]
    fn net_flags_leave_the_process_block_alone() {
        // given: --socks and --no-exit, a process block with SOCKS off
        let main = flagged_relaunch(Some(false), Some(true));
        let mut s = Settings::default();
        s.process.socks_enabled = false;
        let process_before = s.process.clone();
        // when: the flags are laid over the settings of a slot
        main.apply_net_flags(&mut s);
        // then: the process block is the one it was
        assert_eq!(s.process, process_before);
    }

    #[test]
    fn net_flags_never_put_the_key_into_the_settings() {
        // given: a process started with --key
        let main = flagged_relaunch(None, None);
        let mut s = Settings::default();
        // when: the flags are laid over the settings of a slot
        main.apply_net_flags(&mut s);
        // then: the settings still have no key
        assert_eq!(s.net.network_key, None);
    }

    fn flagged_relaunch(exit_enabled: Option<bool>, socks_enabled: Option<bool>) -> Relaunch {
        Relaunch {
            file: None,
            cli: CliOverrides {
                network_key: Some("main-key".into()),
                exit_enabled,
                socks_enabled,
                ..CliOverrides::default()
            },
            secret: test_secret(3),
            initial_domains: Vec::new(),
            blocked_domains: Vec::new(),
            test_net: None,
        }
    }

    fn bare_run() -> RunConfig {
        config_file::prepare(&ConfigSource::Disabled, &CliOverrides::default(), false, 0).unwrap()
    }
}

struct Fixture {
    relaunch: Relaunch,
    path: std::path::PathBuf,
    dir: std::path::PathBuf,
}

impl Fixture {
    fn rewrite_interval(&self, interval: u64) {
        let net = NetSettings {
            probe_interval_sec: interval,
            ..NetSettings::default()
        };
        save_net_file(&self.path, Some(1), 1_790_000_001, 5, &test_secret(9), &net)
            .expect("rewrite the slot file");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

/// A slot file `<tmp>/nets/ab2c3d/config.json` with the given probe interval and a `Relaunch` over it.
fn setup(interval: u64) -> Fixture {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "mesh-node-ctl-slot-test-{}-{nanos}",
        std::process::id()
    ));
    let path = dir.join("nets").join("ab2c3d").join("config.json");
    let net = NetSettings {
        probe_interval_sec: interval,
        ..NetSettings::default()
    };
    save_net_file(&path, None, 1_790_000_000, 5, &test_secret(9), &net)
        .expect("write the slot file");
    let file: std::sync::Arc<dyn SettingsFile> = std::sync::Arc::new(NetFile {
        path: path.clone(),
        created_unix: 5,
    });
    let relaunch = Relaunch {
        file: Some(file),
        cli: CliOverrides::default(),
        secret: test_secret(3),
        initial_domains: Vec::new(),
        blocked_domains: Vec::new(),
        test_net: None,
    };
    Fixture {
        relaunch,
        path,
        dir,
    }
}

/// A `Relaunch` over a slot file at `path` (the file itself is not created).
fn slot_relaunch(path: &std::path::Path) -> Relaunch {
    let file: std::sync::Arc<dyn SettingsFile> = std::sync::Arc::new(NetFile {
        path: path.to_path_buf(),
        created_unix: 6,
    });
    Relaunch {
        file: Some(file),
        cli: CliOverrides::default(),
        secret: test_secret(4),
        initial_domains: Vec::new(),
        blocked_domains: Vec::new(),
        test_net: None,
    }
}
