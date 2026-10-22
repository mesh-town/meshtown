use super::*;
use crate::config_file::{NetSettings, ProcessSettings};
use crate::net::NodeSecret;

#[test]
fn merge_live_splits_live_and_deferred_paths() {
    let mut effective_settings = Settings::default();
    let from = Settings {
        net: NetSettings { exit_enabled: !effective_settings.net.exit_enabled, ..NetSettings::default() },
        ..Settings::default()
    };
    let checks_before = effective_settings.net.checks_enabled;
    let paths: [&'static str; 2] = ["exit.enabled", "web.port"];
    let (live, deferred) = copy_live_paths(&mut effective_settings, &from, &paths);
    assert_eq!(live, vec!["exit.enabled"]);
    assert_eq!(deferred, vec!["web.port"]);
    assert_eq!(effective_settings.net.exit_enabled, from.net.exit_enabled);
    assert_eq!(
        effective_settings.net.checks_enabled, checks_before,
        "a field outside paths does not change"
    );
}

#[test]
fn route_exits_is_live() {
    assert!(
        crate::config_file::fields::live_paths().contains(&"route.exits"),
        "route.exits must be a live path"
    );
    assert!(
        !crate::config_file::fields::live_paths().contains(&"route.all_traffic"),
        "route.all_traffic was removed from the live paths"
    );

    let mut effective_settings = Settings::default();
    let a = NodeSecret::from_bytes(&[1u8; 32]).peer_id();
    let from = Settings {
        net: NetSettings { route_exits: vec![a], ..NetSettings::default() },
        ..Settings::default()
    };
    let paths: [&'static str; 1] = ["route.exits"];

    let (live, deferred) = copy_live_paths(&mut effective_settings, &from, &paths);

    assert_eq!(live, vec!["route.exits"]);
    assert!(
        deferred.is_empty(),
        "changing route.exits must not require a restart"
    );
    assert_eq!(effective_settings.net.route_exits, from.net.route_exits);
}

#[test]
fn ports_paths_are_live() {
    // given: `ports.published` and `ports.opened` changed.
    let mut effective_settings = Settings::default();
    let a = NodeSecret::from_bytes(&[1u8; 32]).peer_id();
    let from = Settings {
        net: NetSettings {
            ports_published: vec![crate::ports::Published {
                port: 8080,
                name: "web".into(),
                audience: crate::ports::Audience::All,
            }],
            ports_opened: vec![crate::ports::Opened { node: a, port: 80, local: 8081 }],
            ..NetSettings::default()
        },
        ..Settings::default()
    };
    let paths: [&'static str; 2] = ["ports.published", "ports.opened"];

    // when: copy_live_paths / then both are live, not in `deferred`.
    let (live, deferred) = copy_live_paths(&mut effective_settings, &from, &paths);

    assert_eq!(live, vec!["ports.published", "ports.opened"]);
    assert!(deferred.is_empty());
    assert_eq!(effective_settings.net.ports_published, from.net.ports_published);
    assert_eq!(effective_settings.net.ports_opened, from.net.ports_opened);
}

mod live_set {
    use std::collections::BTreeSet;

    #[test]
    fn live_set_golden() {
        // given: the original set of live paths; then: exactly these 9
        let expected: BTreeSet<&str> = [
            "exit.enabled",
            "checks.enabled",
            "checks.interval_sec",
            "probe.interval_sec",
            "route.exits",
            "ports.published",
            "ports.opened",
            "log.ring_lines",
            "node.name",
        ]
        .into_iter()
        .collect();
        let actual: BTreeSet<&str> = crate::config_file::fields::live_paths().into_iter().collect();
        assert_eq!(actual, expected, "the set of live paths changed");
        assert_eq!(crate::config_file::fields::live_paths().len(), 9, "the list has no duplicates");
    }
}

mod handles_and_copy {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn handles_equal_live() {
        // given: handles and live paths; then: the sets are equal, no duplicates
        let handles: BTreeSet<&str> = HANDLES.iter().map(|h| h.0).collect();
        let live: BTreeSet<&str> = fields::live_paths().into_iter().collect();
        assert_eq!(handles, live);
        assert_eq!(HANDLES.len(), live.len());
    }

    #[test]
    fn merge_live_copies_each_live_field() {
        // given: `from` differs in all fields; when copy_live_paths over the live paths
        let from = crate::config_file::test_support::all_changed();
        let mut effective_settings = Settings::default();
        let paths = fields::live_paths();
        let (live, deferred) = copy_live_paths(&mut effective_settings, &from, &paths);
        // then: all live ones are copied, the rest stayed at their defaults
        assert_eq!(live, paths);
        assert!(deferred.is_empty());
        assert_eq!(effective_settings.net.exit_enabled, from.net.exit_enabled);
        assert_eq!(effective_settings.net.checks_enabled, from.net.checks_enabled);
        assert_eq!(effective_settings.net.check_interval_sec, from.net.check_interval_sec);
        assert_eq!(effective_settings.net.probe_interval_sec, from.net.probe_interval_sec);
        assert_eq!(effective_settings.net.route_exits, from.net.route_exits);
        assert_eq!(effective_settings.net.ports_published, from.net.ports_published);
        assert_eq!(effective_settings.net.ports_opened, from.net.ports_opened);
        assert_eq!(effective_settings.process.log_ring_lines, from.process.log_ring_lines);
        assert_eq!(effective_settings.net.node_name, from.net.node_name);
        let rest = Settings {
            process: ProcessSettings { socks_port: 1, ..effective_settings.process.clone() },
            ..effective_settings.clone()
        };
        assert_ne!(rest, effective_settings);
        assert_eq!(effective_settings.process.socks_port, Settings::default().process.socks_port, "a non-live field is not copied");
    }
}
