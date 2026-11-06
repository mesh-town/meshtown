use crate::config::config_file::test_support::test_secret;
use crate::config::config_file::{NetSettings, Settings};
use crate::members::network_key::{NetworkKey, NetworkKeyInput};
use crate::node::NodeConfig;
use crate::transport::net::NodeSecret;

#[test]
fn node_config_carries_identity_secret() {
    let key = key();
    let cfg = NodeConfig::from_settings(&Settings::default(), key, test_secret());
    assert!(cfg.identity_secret.is_some());
}
#[test]
fn node_config_keeps_node_name_and_secret() {
    // given: settings with a node name and a node secret
    let key = key();
    let s = Settings {
        net: NetSettings {
            node_name: Some("n".to_string()),
            ..NetSettings::default()
        },
        ..Settings::default()
    };
    // when: the settings are turned into a node config
    let cfg = NodeConfig::from_settings(&s, key, test_secret());
    // then: the node name arrived in settings, and the secret is exactly the one that was passed
    assert_eq!(
        cfg.settings.node_name,
        Some("n".to_string()),
        "node_name was lost"
    );
    assert!(
        cfg.identity_secret.is_some(),
        "the node secret was not put into the config"
    );
    assert!(
        cfg.identity_secret.map(|k| k.peer_id()) == Some(test_secret().peer_id()),
        "the config holds a secret other than the one passed"
    );
}
#[test]
fn fills_process_part_from_process_block() {
    // given: a process block with a SOCKS port and memory thresholds, a node name in the network block
    let key = key();
    let s = Settings {
        process: crate::config::config_file::ProcessSettings {
            socks_enabled: false,
            socks_port: 1090,
            node_memory_warn_mib: 300,
            node_memory_exit_mib: 800,
            ..crate::config::config_file::ProcessSettings::default()
        },
        net: NetSettings {
            node_name: Some("x".to_string()),
            ..NetSettings::default()
        },
    };
    // when: the settings are turned into a node config
    let cfg = NodeConfig::from_settings(&s, key, test_secret());
    // then: the process part came from the process block, the network part from net, the secret is the one passed
    assert_eq!(cfg.settings, s.net);
    assert_eq!(
        cfg.identity_secret.map(|k| k.peer_id()),
        Some(test_secret().peer_id())
    );
}
#[test]
fn node_config_carries_net() {
    // given: settings with the chosen sync "mem"
    let key = key();
    let net = crate::transport::net::NetChoice {
        sync: "mem".to_string(),
        ..Settings::default().net.net_choice
    };
    let s = Settings {
        net: NetSettings {
            net_choice: net,
            ..NetSettings::default()
        },
        ..Settings::default()
    };
    // when: the settings are turned into a node config
    let cfg = NodeConfig::from_settings(&s, key, test_secret());
    // then: the network choice reached the node
    assert_eq!(
        cfg.settings.net_choice.sync, "mem",
        "net was not carried into NodeConfig"
    );
}

fn key() -> NetworkKeyInput {
    let key = NetworkKey::generate().expect("network key");
    NetworkKeyInput {
        label: None,
        base: key.to_bytes(),
        key,
        from_words: false,
        root: NodeSecret::from_bytes(&[7; 32]).peer_id(),
    }
}
