use super::*;
use crate::config::config_file::test_support::test_secret;
use crate::config::config_file::NetSettings;
use crate::members::network_key::NetworkKey;
use std::net::SocketAddr;
use std::time::Duration;

#[test]
fn defaults_match_config_file_md() {
    let s = Settings::default();
    assert_eq!(s.net.node_name, None);
    assert_eq!(s.net.network_key, None);
    assert!(s.net.exit_enabled);
    assert!(s.process.socks_enabled);
    assert_eq!(s.process.socks_port, 1080);
    assert!(s.net.checks_enabled);
    assert_eq!(s.net.check_interval_sec, 900);
    assert_eq!(
        s.net.doh_server,
        "1.1.1.1:443".parse::<SocketAddr>().unwrap()
    );
    assert_eq!(s.net.provider_dns, None);
    assert_eq!(s.net.probe_interval_sec, 300);
    assert!(
        !s.net.probe_full_cycle,
        "probe.full_cycle is off by default"
    );
    assert!(s.net.route_exits.is_empty(), "route.exits defaults to []");
    assert!(s.process.web_enabled);
    assert_eq!(s.process.web_port, 7777);
    assert_eq!(s.process.log_ring_lines, 2000);
    assert!(!s.process.autostart_enabled);
    assert!(!s.process.full_tunnel_enabled);
}

#[test]
fn intervals_are_seconds() {
    // given: intervals of 300 and 900 seconds (different, so swapped methods fail) and zeros
    let set = Settings {
        net: NetSettings {
            probe_interval_sec: 300,
            check_interval_sec: 900,
            ..NetSettings::default()
        },
        ..Settings::default()
    };
    let zero = Settings {
        net: NetSettings {
            probe_interval_sec: 0,
            check_interval_sec: 0,
            ..NetSettings::default()
        },
        ..Settings::default()
    };
    // when: the methods convert seconds to Duration
    // then: exactly Duration::from_secs of the same numbers, zero gives Duration::ZERO
    assert_eq!(
        set.net.probe_interval(),
        Duration::from_secs(300),
        "probe_interval"
    );
    assert_eq!(
        set.net.check_interval(),
        Duration::from_secs(900),
        "check_interval"
    );
    assert_eq!(
        zero.net.probe_interval(),
        Duration::ZERO,
        "probe_interval at 0"
    );
    assert_eq!(
        zero.net.check_interval(),
        Duration::ZERO,
        "check_interval at 0"
    );
}

#[test]
fn debug_hides_key() {
    let s = Settings {
        net: NetSettings {
            network_key: Some("label--secret-probe-x".into()),
            ..NetSettings::default()
        },
        ..Default::default()
    };
    let text = format!("{:?}", s);
    assert!(
        !text.contains("secret"),
        "the key leaked into Debug: {text}"
    );
    assert!(
        text.contains("<hidden>"),
        "the <hidden> marker is missing: {text}"
    );
}

#[test]
fn node_config_carries_identity_secret() {
    let key = NetworkKey::generate().expect("network key");
    let cfg = Settings::default().node_config(key, test_secret());
    assert!(cfg.identity_secret.is_some());
}

#[test]
fn node_config_keeps_node_name_and_secret() {
    // given: settings with a node name and a node secret
    let key = NetworkKey::generate().expect("network key");
    let s = Settings {
        net: NetSettings {
            node_name: Some("n".to_string()),
            ..NetSettings::default()
        },
        ..Settings::default()
    };
    // when: the settings are turned into a node config
    let cfg = s.node_config(key, test_secret());
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
    let key = NetworkKey::generate().expect("network key");
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
    let cfg = s.node_config(key, test_secret());
    // then: the process part came from the process block, the network part from net, the secret is the one passed
    assert_eq!(cfg.settings, s.net);
    assert_eq!(
        cfg.identity_secret.map(|k| k.peer_id()),
        Some(test_secret().peer_id())
    );
}

#[test]
fn web_access_default_is_localnet() {
    assert_eq!(
        Settings::default().process.web_access,
        ListenAccess::Localnet
    );
}

#[test]
fn web_access_roundtrip() {
    for a in [ListenAccess::Localhost, ListenAccess::Localnet] {
        assert_eq!(ListenAccess::parse(a.as_str()), Some(a));
    }
    assert_eq!(ListenAccess::parse("anywhere"), None);
}

#[test]
fn probe_full_cycle_default_false_and_read_true() {
    let p = crate::config::config_file::parse_text(r#"{"config":{"version":1}}"#).expect("parsing");
    assert!(!p.settings.net.probe_full_cycle);
    let p = crate::config::config_file::parse_text(
        r#"{"config":{"version":1,"probe":{"full_cycle":true}}}"#,
    )
    .expect("parsing");
    assert!(p.settings.net.probe_full_cycle);
    assert_eq!(
        p.settings.net.probe_interval_sec, 300,
        "the neighboring field of the section is at its default"
    );
    assert_eq!(p.field_errors, vec![]);
    assert!(
        p.unknown_paths.is_empty(),
        "full_cycle must not be unknown: {:?}",
        p.unknown_paths
    );
}

#[test]
fn debug_shows_net() {
    // given: a non-default network choice — sync = "mem"
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
    // when: Debug of the settings
    let text = format!("{:?}", s);
    // then: the output has the net field and the chosen sync (the substring "mem" would also be found in `node_memory_*`)
    assert!(
        text.contains("net.sync"),
        "the net.sync line is missing: {text}"
    );
    assert!(
        text.contains(r#"net.sync: "mem""#),
        "the chosen sync is missing: {text}"
    );
}

#[test]
fn node_config_carries_net() {
    // given: settings with the chosen sync "mem"
    let key = NetworkKey::generate().expect("network key");
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
    let cfg = s.node_config(key, test_secret());
    // then: the network choice reached the node
    assert_eq!(
        cfg.settings.net_choice.sync, "mem",
        "net was not carried into NodeConfig"
    );
}
