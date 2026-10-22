//! Tests of `ProxyState`: the source of the proxy menu item (the "off" reason is not lost).

use super::*;

fn on(addr: &str) -> ProxyState {
    ProxyState::On(addr.to_string())
}

fn socks(enabled: bool, listen: Option<&str>) -> TraySocks {
    TraySocks {
        enabled,
        listen: listen.map(str::to_string),
    }
}

#[test]
fn of_plan_port_is_loopback_address() {
    assert_eq!(ProxyState::of_plan(Ok(1080)), on("127.0.0.1:1080"));
}

#[test]
fn of_plan_flag_is_off_by_flag() {
    assert_eq!(
        ProxyState::of_plan(Err(ProxyOff::NoSocksFlag)),
        ProxyState::Off(ProxyOff::NoSocksFlag)
    );
}

#[test]
fn of_plan_settings_is_off_by_settings() {
    assert_eq!(
        ProxyState::of_plan(Err(ProxyOff::DisabledInSettings)),
        ProxyState::Off(ProxyOff::DisabledInSettings)
    );
}

#[test]
fn of_snapshot_enabled_with_listen_is_on() {
    let snap = socks(true, Some("127.0.0.1:1080"));
    assert_eq!(ProxyState::of_snapshot(&snap), on("127.0.0.1:1080"));
}

#[test]
fn of_snapshot_disabled_is_off_by_settings() {
    let snap = socks(false, Some("127.0.0.1:1080"));
    assert_eq!(
        ProxyState::of_snapshot(&snap),
        ProxyState::Off(ProxyOff::DisabledInSettings)
    );
}

#[test]
fn of_snapshot_enabled_without_listen_is_off_by_settings() {
    assert_eq!(
        ProxyState::of_snapshot(&socks(true, None)),
        ProxyState::Off(ProxyOff::DisabledInSettings)
    );
}

#[test]
fn with_flag_of_own_flag_turns_off_into_flag() {
    let own = ProxyState::Off(ProxyOff::NoSocksFlag);
    let got = ProxyState::Off(ProxyOff::DisabledInSettings).with_flag_reason_from(&own);
    assert_eq!(got, ProxyState::Off(ProxyOff::NoSocksFlag));
}

#[test]
fn with_flag_of_other_own_keeps_settings() {
    let settings = ProxyState::Off(ProxyOff::DisabledInSettings);
    for own in [settings.clone(), on("127.0.0.1:1080")] {
        assert_eq!(settings.clone().with_flag_reason_from(&own), settings);
    }
}

#[test]
fn with_flag_of_keeps_on_for_any_own() {
    let owns = [
        ProxyState::Off(ProxyOff::NoSocksFlag),
        ProxyState::Off(ProxyOff::DisabledInSettings),
        on("127.0.0.1:1081"),
    ];
    for own in owns {
        assert_eq!(
            on("127.0.0.1:1080").with_flag_reason_from(&own),
            on("127.0.0.1:1080")
        );
    }
}
