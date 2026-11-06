use super::*;
use std::time::Duration;

fn with_key(key: Option<&str>) -> NetSettings {
    NetSettings {
        network_key: key.map(str::to_string),
        ..NetSettings::default()
    }
}

#[test]
fn debug_shows_none_without_network_key() {
    // given: there is no key
    let n = with_key(None);
    // when: printing {:?}
    let t = format!("{n:?}");
    // then: None is visible, the marker is absent
    assert!(t.contains("network_key: None"), "{t}");
    assert!(!t.contains("<hidden>"), "{t}");
}

#[test]
fn intervals_are_seconds_of_net_block() {
    // given: intervals in seconds
    let n = NetSettings {
        probe_interval_sec: 77,
        check_interval_sec: 33,
        ..NetSettings::default()
    };
    // when/then: the methods return the same seconds
    assert_eq!(n.probe_interval(), Duration::from_secs(77));
    assert_eq!(n.check_interval(), Duration::from_secs(33));
}

#[test]
fn exit_is_off_by_default_only_on_android() {
    assert!(!default_exit_enabled(true));
    assert!(default_exit_enabled(false));
}
