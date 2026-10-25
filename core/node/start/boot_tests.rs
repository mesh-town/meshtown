use super::*;
use crate::node::identity::TemporaryReason;

fn pid(n: u8) -> PeerId {
    PeerId::from_bytes(&[n; 32]).unwrap()
}

#[test]
fn lines_hold_only_the_banner() {
    // given: a persistent identity, the name passed the check
    let id = pid(1);
    // when: the lines are built
    let lines = identity_lines(
        "abcd1234",
        id,
        "tree",
        Some(Ok("tree")),
        IdentityState::Persistent,
    );
    // then: only the banner
    let want = format!(
        "meshtown {}, network {} abcd1234, node {} \"tree\" ({}), identity persistent",
        env!("CARGO_PKG_VERSION"),
        crate::node::output::NET_NAME,
        id.fmt_short(),
        HEXLOWER.encode(id.as_bytes()),
    );
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0], want);
}

#[test]
fn bad_config_name_adds_second_line() {
    // given: the name from the config did not fit
    let lines = identity_lines(
        "abcd1234",
        pid(1),
        "tree",
        Some(Err("too long")),
        IdentityState::Persistent,
    );
    // then: the second line has the reason and the name taken
    assert_eq!(lines.len(), 2);
    assert_eq!(
        lines[1],
        "name from the settings file (node.name) is not valid: too long; using name \"tree\" for this run"
    );
}

#[test]
fn temporary_identity_adds_its_text() {
    // given: a temporary identity
    let identity = IdentityState::Temporary(TemporaryReason::Declined);
    let lines = identity_lines("abcd1234", pid(1), "tree", None, identity);
    // then: the identity text follows the banner
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[1], identity.warning_text().unwrap());
}
