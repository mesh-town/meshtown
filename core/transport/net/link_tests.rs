//! Tests for `net/link.rs`: `LinkInfo::hint` — which address hint comes out of the connection's chosen path.

use super::*;
use crate::transport::net::addr::{AddrHint, RelayRef};
use std::net::SocketAddr;

const REMOTE: &str = "203.0.113.9:4433";
const RELAY_URL: &str = "https://relay.example.com/";

#[test]
fn hint_direct_with_remote_is_ip() {
    // given: a direct path with a known address of the peer
    let info = link(PathKind::Direct, Some(remote()));
    // when: we ask for a hint
    let hint = info.hint();
    // then: it is an Ip with the same address
    assert_eq!(hint, Some(AddrHint::Ip(remote())));
}

#[test]
fn hint_direct_without_remote_is_none() {
    // given: a direct path, the transport does not know the peer's address (mem)
    let info = link(PathKind::Direct, None);
    // when: we ask for a hint
    let hint = info.hint();
    // then: there is no hint
    assert_eq!(hint, None, "with no address there is nothing to hint");
}

#[test]
fn hint_relayed_is_relay() {
    // given: a path through a relay by url
    let info = link(PathKind::Relayed(relay()), None);
    // when: we ask for a hint
    let hint = info.hint();
    // then: it is a Relay with the same relay
    assert_eq!(hint, Some(AddrHint::Relay(relay())));
}

#[test]
fn hint_relayed_ignores_remote() {
    // given: a path through a relay, and the address is known too
    let info = link(PathKind::Relayed(relay()), Some(remote()));
    // when: we ask for a hint
    let hint = info.hint();
    // then: the path kind decides, not the presence of an address: Relay, not Ip
    assert_eq!(hint, Some(AddrHint::Relay(relay())));
}

#[test]
fn hint_unknown_is_none() {
    // given: no path chosen, the address is known just in case
    let info = link(PathKind::Unknown, Some(remote()));
    // when: we ask for a hint
    let hint = info.hint();
    // then: there is no hint
    assert_eq!(hint, None, "an unknown path must have no hint");
}

// ---- helpers ----

fn link(path: PathKind, remote: Option<SocketAddr>) -> LinkInfo {
    LinkInfo {
        path,
        remote,
        ..LinkInfo::default()
    }
}

fn remote() -> SocketAddr {
    REMOTE.parse().expect("the address in the constant parses")
}

fn relay() -> RelayRef {
    RelayRef::Url(RELAY_URL.to_string())
}
