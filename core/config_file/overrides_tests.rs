use super::*;
use crate::config_file::settings::{Settings, TransportMode, ListenAccess};
use crate::config_file::{NetSettings, ProcessSettings};
use crate::config_file::test_support::sample_settings;

#[test]
fn apply_is_per_field() {
    // Settings "from the file": port 1081, check interval 900.
    let mut s = Settings {
        process: ProcessSettings {
            socks_port: 1081,
            ..ProcessSettings::default()
        },
        net: NetSettings {
            check_interval_sec: 900,
            ..NetSettings::default()
        },
    };
    let cli = CliOverrides {
        socks_port: Some(2000),
        ..CliOverrides::default()
    };
    let paths = cli.apply(&mut s);
    assert_eq!(s.process.socks_port, 2000);
    assert_eq!(s.net.check_interval_sec, 900);
    assert_eq!(paths, vec!["socks.port"]);
}

#[test]
fn apply_empty_changes_nothing() {
    let before = sample_settings();
    let mut s = before.clone();
    let paths = CliOverrides::default().apply(&mut s);
    assert_eq!(s, before);
    assert!(paths.is_empty(), "paths: {paths:?}");
}

#[test]
fn apply_mode() {
    let mut s = Settings::default();
    let cli = CliOverrides {
        transport_mode: Some(TransportMode::NoRelay),
        ..CliOverrides::default()
    };
    let paths = cli.apply(&mut s);
    assert_eq!((s.net.uses_relay(), s.net.is_relay_only()), (false, false));
    assert_eq!(paths, vec!["transport.mode"]);
}

#[test]
fn apply_probe_full_cycle() {
    let mut s = Settings::default();
    let cli = CliOverrides {
        probe_full_cycle: Some(true),
        ..CliOverrides::default()
    };
    let paths = cli.apply(&mut s);
    assert!(s.net.probe_full_cycle);
    assert_eq!(paths, vec!["probe.full_cycle"]);
}

#[test]
fn override_paths_are_in_table() {
    // given: all flags are set; when apply; then every path is in the table (13 of them)
    let cli = CliOverrides {
        network_key: Some("k".into()),
        transport_mode: Some(TransportMode::NoRelay),
        exit_enabled: Some(true),
        socks_enabled: Some(true),
        socks_port: Some(2000),
        socks_access: Some(ListenAccess::Localnet),
        checks_enabled: Some(true),
        check_interval_sec: Some(60),
        doh_server: Some("1.1.1.1:443".parse().unwrap()),
        provider_dns: Some("1.1.1.1:53".parse().unwrap()),
        probe_interval_sec: Some(60),
        probe_full_cycle: Some(true),
        web_port: Some(2001),
    };
    let paths = cli.apply(&mut Settings::default());
    assert_eq!(paths.len(), 13);
    for p in paths {
        assert!(crate::config_file::fields::field_by_path(p).is_some(), "{p} is not in the table");
    }
}

#[test]
fn socks_access_flag_overrides_file() {
    // given: the file says localhost; --socks-access localnet is given
    let mut s = Settings::default();
    assert_eq!(s.process.socks_access, ListenAccess::Localhost);
    let cli = CliOverrides { socks_access: Some(ListenAccess::Localnet), ..CliOverrides::default() };
    // when: flags over the file; then: localnet mode and the path socks.access
    let paths = cli.apply(&mut s);
    assert_eq!(s.process.socks_access, ListenAccess::Localnet);
    assert_eq!(paths, vec!["socks.access"]);
}
