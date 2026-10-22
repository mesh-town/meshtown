use super::*;
use crate::config::config_file::{NetSettings, TransportMode};
use crate::node::identity::TemporaryReason;

fn pid(n: u8) -> PeerId {
    PeerId::from_bytes(&[n; 32]).unwrap()
}

fn settings(mode: TransportMode) -> NetSettings {
    let mut settings = NodeConfig::for_loopback_test(NetworkKey::generate().unwrap()).settings;
    settings.transport_mode = mode;
    settings
}

#[test]
fn lines_start_with_banner_and_end_with_mode() {
    // given: a persistent identity, the name passed the check
    let id = pid(1);
    let settings = settings(TransportMode::NoRelay);
    // when: the lines are built
    let lines = identity_lines(
        &settings,
        "abcd1234",
        id,
        "tree",
        Some(Ok("tree")),
        IdentityState::Persistent,
    );
    // then: the banner and the mode
    let want = format!(
        "meshtown {}, network {} abcd1234, node {} \"tree\" ({}), identity persistent",
        env!("CARGO_PKG_VERSION"),
        crate::node::output::NET_NAME,
        id.fmt_short(),
        HEXLOWER.encode(id.as_bytes()),
    );
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], want);
    assert_eq!(lines[1], "mode: no relay");
}

#[test]
fn bad_config_name_adds_second_line() {
    // given: the name from the config did not fit
    let lines = identity_lines(
        &settings(TransportMode::NoRelay),
        "abcd1234",
        pid(1),
        "tree",
        Some(Err("too long")),
        IdentityState::Persistent,
    );
    // then: the second line has the reason and the name taken
    assert_eq!(lines.len(), 3);
    assert_eq!(
        lines[1],
        "name from the settings file (node.name) is not valid: too long; using name \"tree\" for this run"
    );
}

#[test]
fn temporary_identity_adds_its_text() {
    // given: a temporary identity
    let identity = IdentityState::Temporary(TemporaryReason::Declined);
    let lines = identity_lines(
        &settings(TransportMode::NoRelay),
        "abcd1234",
        pid(1),
        "tree",
        None,
        identity,
    );
    // then: the identity text comes before the mode
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[1], identity.warning_text().unwrap());
    assert_eq!(lines[2], "mode: no relay");
}

#[test]
fn mode_names_each_transport_mode() {
    assert_eq!(
        mode_line(&settings(TransportMode::NoRelay)),
        "mode: no relay"
    );
    assert_eq!(
        mode_line(&settings(TransportMode::RelayOnly)),
        "mode: relay only"
    );
    assert_eq!(
        mode_line(&settings(TransportMode::DirectAndRelay)),
        "mode: direct + relay"
    );
}
