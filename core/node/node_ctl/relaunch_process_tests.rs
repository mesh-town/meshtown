//! `Relaunch::node_config`: the process part of the node config (SOCKS, memory thresholds) and the network choice.

use super::*;
use crate::config::config_file::{NetSettings, ProcessSettings};

fn secret() -> NodeSecret {
    NodeSecret::from_bytes(&[40u8; 32])
}

fn relaunch(socks_enabled: Option<bool>) -> Relaunch {
    Relaunch {
        file: None,
        cli: CliOverrides {
            socks_enabled,
            ..CliOverrides::default()
        },
        secret: secret(),
        initial_domains: Vec::new(),
        blocked_domains: Vec::new(),
        test_net: None,
    }
}

fn settings_with_process_block() -> Settings {
    Settings {
        process: ProcessSettings {
            socks_port: 1091,
            node_memory_warn_mib: 350,
            node_memory_exit_mib: 850,
            ..ProcessSettings::default()
        },
        net: NetSettings::default(),
    }
}

fn key() -> NetworkKey {
    NetworkKey::from_bytes([41u8; 32])
}

#[test]
fn test_net_replaces_only_choice() {
    // given: the tests' mem network and a process block with port 1091
    let s = settings_with_process_block();
    let mut r = relaunch(None);
    r.test_net = Some(crate::transport::net::mem::process_net());
    // when: rebuilding the node config
    let cfg = r.node_config(&s, key());
    // then: the network choice — as with loopback, the process part and other network fields — from the settings
    let loopback = NodeConfig::for_loopback_test(key()).settings.net_choice;
    assert_eq!(cfg.settings.net_choice, loopback);
    assert_eq!(
        cfg.settings,
        NetSettings {
            net_choice: loopback,
            ..s.net
        }
    );
}
