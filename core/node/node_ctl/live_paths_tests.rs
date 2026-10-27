use super::*;
use crate::config::config_file::{NetSettings, ProcessSettings};
use crate::ports_sharing::types::Proto;
use crate::transport::net::NodeSecret;

#[test]
fn merge_live_splits_live_and_deferred_paths() {
    let mut effective_settings = Settings::default();
    let from = Settings {
        net: NetSettings {
            exit_enabled: !effective_settings.net.exit_enabled,
            ..NetSettings::default()
        },
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
        crate::config::config_file::fields::live_paths().contains(&"route.exits"),
        "route.exits must be a live path"
    );
    assert!(
        !crate::config::config_file::fields::live_paths().contains(&"route.all_traffic"),
        "route.all_traffic was removed from the live paths"
    );

    let mut effective_settings = Settings::default();
    let a = NodeSecret::from_bytes(&[1u8; 32]).peer_id();
    let from = Settings {
        net: NetSettings {
            route_exits: vec![a],
            ..NetSettings::default()
        },
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
fn proxy_net_is_live_and_deferred_never() {
    // given: only `proxy.net` changed in the incoming settings
    let mut effective_settings = Settings::default();
    let from = crate::config::config_file::test_support::all_changed();
    let paths: [&'static str; 1] = ["proxy.net"];

    // when: copy_live_paths
    let (live, deferred) = copy_live_paths(&mut effective_settings, &from, &paths);

    // then: the path is live, nothing waits for a restart, the value moved
    assert_eq!(live, vec!["proxy.net"]);
    assert!(
        deferred.is_empty(),
        "changing proxy.net must not require a restart"
    );
    assert_eq!(effective_settings.process.proxy_net, from.process.proxy_net);
}

#[test]
fn ports_paths_are_live() {
    // given: `ports.published` and `ports.opened` changed.
    let mut effective_settings = Settings::default();
    let a = NodeSecret::from_bytes(&[1u8; 32]).peer_id();
    let from = Settings {
        net: NetSettings {
            ports_published: vec![crate::ports_sharing::Published {
                proto: Proto::Tcp,
                port: 8080,
                name: "web".into(),
                audience: crate::ports_sharing::Audience::All,
            }],
            ports_opened: vec![crate::ports_sharing::Opened {
                proto: Proto::Tcp,
                node: a,
                port: 80,
                local: 8081,
            }],
            ..NetSettings::default()
        },
        ..Settings::default()
    };
    let paths: [&'static str; 2] = ["ports.published", "ports.opened"];

    // when: copy_live_paths / then both are live, not in `deferred`.
    let (live, deferred) = copy_live_paths(&mut effective_settings, &from, &paths);

    assert_eq!(live, vec!["ports.published", "ports.opened"]);
    assert!(deferred.is_empty());
    assert_eq!(
        effective_settings.net.ports_published,
        from.net.ports_published
    );
    assert_eq!(effective_settings.net.ports_opened, from.net.ports_opened);
}

mod live_set {
    use std::collections::BTreeSet;

    #[test]
    fn live_set_golden() {
        // given: the set of live paths; then: exactly these 12
        let expected: BTreeSet<&str> = [
            "exit.enabled",
            "checks.enabled",
            "checks.interval_sec",
            "probe.interval_sec",
            "route.exits",
            "ports.published",
            "ports.opened",
            "log.ring_lines",
            "exit.max_streams",
            "node.name",
            "proxy.net",
            "socks.strict",
        ]
        .into_iter()
        .collect();
        let actual: BTreeSet<&str> = crate::config::config_file::fields::live_paths()
            .into_iter()
            .collect();
        assert_eq!(actual, expected, "the set of live paths changed");
        assert_eq!(
            crate::config::config_file::fields::live_paths().len(),
            12,
            "the list has no duplicates"
        );
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
        assert_eq!(HANDLES.len(), 12, "one handle per live path");
    }

    #[test]
    fn proxy_net_has_exactly_one_handle() {
        // given: the handle list; then: `proxy.net` is listed once
        let count = HANDLES.iter().filter(|h| h.0 == "proxy.net").count();
        assert_eq!(count, 1, "`proxy.net` must have exactly one handle");
    }

    fn assert_process_fields_copied(effective_settings: &Settings, from: &Settings) {
        assert_eq!(
            effective_settings.process.log_ring_lines,
            from.process.log_ring_lines
        );
        assert_eq!(
            effective_settings.process.exit_max_streams, from.process.exit_max_streams,
            "exit.max_streams is copied as a live field"
        );
        assert_ne!(
            effective_settings.process.exit_max_streams,
            Settings::default().process.exit_max_streams,
            "the probe value differs from the default, so the copy is observable"
        );
        assert_eq!(
            effective_settings.process.proxy_net, from.process.proxy_net,
            "proxy.net is copied as a live field"
        );
        assert_ne!(
            effective_settings.process.proxy_net,
            Settings::default().process.proxy_net,
            "the probe value differs from the default, so the copy is observable"
        );
    }

    #[test]
    fn merge_live_copies_each_live_field() {
        // given: `from` differs in all fields; when copy_live_paths over the live paths
        let from = crate::config::config_file::test_support::all_changed();
        let mut effective_settings = Settings::default();
        let paths = fields::live_paths();
        let (live, deferred) = copy_live_paths(&mut effective_settings, &from, &paths);
        // then: all live ones are copied, the rest stayed at their defaults
        assert_eq!(live, paths);
        assert!(deferred.is_empty());
        assert_eq!(effective_settings.net.exit_enabled, from.net.exit_enabled);
        assert_eq!(
            effective_settings.net.checks_enabled,
            from.net.checks_enabled
        );
        assert_eq!(
            effective_settings.net.check_interval_sec,
            from.net.check_interval_sec
        );
        assert_eq!(
            effective_settings.net.probe_interval_sec,
            from.net.probe_interval_sec
        );
        assert_eq!(effective_settings.net.route_exits, from.net.route_exits);
        assert_eq!(
            effective_settings.net.ports_published,
            from.net.ports_published
        );
        assert_eq!(effective_settings.net.ports_opened, from.net.ports_opened);
        assert_eq!(effective_settings.net.node_name, from.net.node_name);
        assert_process_fields_copied(&effective_settings, &from);
        let rest = Settings {
            process: ProcessSettings {
                socks_port: 1,
                ..effective_settings.process.clone()
            },
            ..effective_settings.clone()
        };
        assert_ne!(rest, effective_settings);
        assert_eq!(
            effective_settings.process.socks_port,
            Settings::default().process.socks_port,
            "a non-live field is not copied"
        );
    }
}

mod strict_handle {
    use super::*;

    #[test]
    fn strict_handle_sets_pac_flag() {
        // given: settings asking for strict mode; when: the handle's function is applied
        let on = Settings {
            process: ProcessSettings {
                socks_strict: true,
                ..ProcessSettings::default()
            },
            ..Settings::default()
        };
        let seen = std::cell::Cell::new(None);
        apply_pac_strict(&on, |v| seen.set(Some(v)));
        assert_eq!(seen.get(), Some(true));
        // then: the default settings hand over `false`
        apply_pac_strict(&Settings::default(), |v| seen.set(Some(v)));
        assert_eq!(seen.get(), Some(false));
    }
}

mod exit_max_streams_handle {
    use super::*;
    use crate::exit::ExitLimits;

    #[test]
    fn exit_max_streams_handle_sets_the_limit() {
        // given: a limiter with the cap 5 and settings that ask for 7
        let limits = ExitLimits::fixed(5);
        let settings = settings_with_exit_max_streams(7);

        // when: the handle's function is applied
        apply_exit_max_streams(&limits, &settings);

        // then: the cap follows the setting
        assert_eq!(limits.max_streams(), 7);
    }

    #[test]
    fn zero_setting_goes_back_to_the_automatic_cap() {
        // given: a limiter configured with the setting 9 and the automatic cap 3
        let limits = ExitLimits::fixed(1);
        limits.configure(9, 3, 0);

        // when: the setting becomes 0
        apply_exit_max_streams(&limits, &settings_with_exit_max_streams(0));

        // then: the automatic cap is back
        assert_eq!(limits.max_streams(), 3);
    }

    fn settings_with_exit_max_streams(n: u64) -> Settings {
        Settings {
            process: ProcessSettings {
                exit_max_streams: n,
                ..ProcessSettings::default()
            },
            ..Settings::default()
        }
    }
}
