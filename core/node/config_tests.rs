//! Tests of the `NodeConfig` presets (`config.rs`): `for_loopback_test` and `for_cli`.
//! Read the `cargo test` output as a phrase: `for_loopback_test::keeps_old_preset_values`.

use super::*;
use crate::config::config_file::NetSettings;
use crate::members::network_key::{NetworkKey, NetworkKeyInput};
use crate::site_checks::doh::DEFAULT_DOH;
use crate::transport::net::NetChoice;
use crate::transport::net::NodeSecret;
use std::time::Duration;

mod for_loopback_test {
    use super::*;

    #[test]
    fn keeps_old_preset_values() {
        // given: a network key
        let key = key();
        // when: the preset for loopback tests is built
        let cfg = NodeConfig::for_loopback_test(key);
        let s = &cfg.settings;
        // then: the settings are the same as the copy fields of `NodeConfig` had before moving to `settings`
        assert_eq!(
            s.probe_interval(),
            Duration::from_secs(3600),
            "probe once an hour"
        );
        assert_eq!(
            s.check_interval(),
            Duration::from_secs(900),
            "checks every 15 minutes"
        );
        assert!(!s.checks_enabled, "checks are off");
        assert!(s.exit_enabled, "exit is on");
        assert_eq!(s.doh_server, DEFAULT_DOH, "default DoH server");
        assert_eq!(
            s.net_choice,
            NetChoice {
                transports: names(&["mem"]),
                discovery: names(&["invite", "cache"]),
                sync: "mesh".to_string(),
            },
            "preset network: mem / invite+cache / mesh"
        );
        assert_eq!(s.node_name, None, "node name is not set");
    }

    #[test]
    fn loopback_preset_runs_on_the_mem_network() {
        // given: a network key
        let key = key();
        // when: the preset for loopback tests is built
        let cfg = NodeConfig::for_loopback_test(key);
        // then: the network is mem
        assert_eq!(cfg.settings.net_choice.transports, ["mem"]);
    }

    #[test]
    fn is_open_with_the_fixed_root() {
        // given: a network key
        let key = key();
        // when: the preset for loopback tests is built
        let cfg = NodeConfig::for_loopback_test(key.clone());
        // then: the root is the fixed test node, the reply secret is the key itself, the list is open
        assert_eq!(cfg.net_root, loopback_root(), "fixed root");
        assert_eq!(cfg.reply_secret.to_bytes(), key.to_bytes(), "reply secret");
        assert_eq!(cfg.network_key.to_bytes(), key.to_bytes(), "network key");
        assert!(cfg.open_list, "the list is open");
    }

    #[test]
    fn keeps_non_copy_fields() {
        // given: a network key
        let key = key();
        // when: the preset for loopback tests is built
        let cfg = NodeConfig::for_loopback_test(key);
        // then: the fields not moved into `settings` stayed the same
        assert_eq!(
            cfg.probe_first_delay,
            Duration::ZERO,
            "first probe right away"
        );
        assert!(!cfg.geoip, "we don't ask external sources for the country");
        assert!(cfg.test_net.is_some(), "the mem network must be set");
        assert!(
            cfg.identity_secret.is_none(),
            "secret is not set: a new one on every start"
        );
    }
}

mod for_cli {
    use super::*;

    #[test]
    fn takes_settings_whole() {
        // given: settings that differ from the defaults
        let s = NetSettings {
            probe_interval_sec: 77,
            node_name: Some("x".to_string()),
            ..NetSettings::default()
        };
        // when: the preset for the CLI is built
        let cfg = NodeConfig::for_cli(input(), &s);
        // then: the settings are passed as they are, the rest are the live node's defaults
        assert_eq!(cfg.settings, s, "settings must come in unchanged");
        assert_eq!(
            cfg.probe_first_delay,
            Duration::from_secs(10),
            "first probe after 10 s"
        );
        assert!(cfg.geoip, "the country is asked of external sources");
        assert!(cfg.test_net.is_none(), "the mem network is for tests only");
    }

    #[test]
    fn takes_root_label_and_reply_from_the_key() {
        // given: a key input whose base differs from the key bytes
        let (input, again) = (input(), input());
        // when: the preset for the CLI is built
        let cfg = NodeConfig::for_cli(again, &NetSettings::default());
        // then: the key, the root, the label and the reply secret all come from the input
        assert_eq!(cfg.network_key.to_bytes(), input.key.to_bytes(), "key");
        assert_eq!(cfg.net_root, input.root, "root");
        assert_eq!(cfg.network_label.as_deref(), Some("Office"), "label");
        assert_eq!(
            cfg.reply_secret.to_bytes(),
            input.reply_secret(),
            "reply secret"
        );
        // and: the live node counts the list
        assert!(!cfg.open_list, "the list is counted, not open");
    }

    #[test]
    fn takes_net_settings_by_reference() {
        // given: network settings with a node name and its own network choice
        let n = NetSettings {
            node_name: Some("x".to_string()),
            net_choice: NetChoice {
                transports: names(&["mem"]),
                discovery: names(&["cache"]),
                sync: "mem".to_string(),
            },
            ..NetSettings::default()
        };
        // when: the CLI preset is built by reference
        let cfg = NodeConfig::for_cli(input(), &n);
        // then: the config holds an equal copy, and the original settings stay with the caller
        assert_eq!(
            cfg.settings, n,
            "the copy of settings must equal the original"
        );
        assert_eq!(
            n.node_name.as_deref(),
            Some("x"),
            "the original settings are available"
        );
    }
}

fn key() -> NetworkKey {
    NetworkKey::generate().expect("network key")
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn input() -> NetworkKeyInput {
    NetworkKeyInput {
        label: Some("Office".to_string()),
        key: NetworkKey::from_bytes([4; 32]),
        from_words: false,
        base: [3; 32],
        root: NodeSecret::from_bytes(&[9; 32]).peer_id(),
    }
}
