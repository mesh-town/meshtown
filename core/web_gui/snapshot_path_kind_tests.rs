//! Tests for `snapshot.rs`: `PathKindView::from_net` — the network layer's path kind on the node page.
//! Separate from `snapshot_tests.rs`: that one holds the golden set of `/api/state` keys and is not edited after the golden set was recorded.

use super::PathKindView;
use crate::transport::net::addr::RelayRef;
use crate::transport::net::link::PathKind as NetPath;

#[test]
fn from_net_maps_direct_relayed_unknown() {
    // given: three network-layer path kinds
    let direct = NetPath::Direct;
    let relayed = NetPath::Relayed(RelayRef::Url("https://relay.example.com/".to_string()));
    let unknown = NetPath::Unknown;
    // when: convert each to the page kind
    let got = (
        PathKindView::from_net(&direct),
        PathKindView::from_net(&relayed),
        PathKindView::from_net(&unknown),
    );
    // then: direct — Direct, via relay — Relay, unknown — empty
    assert_eq!(
        got,
        (Some(PathKindView::Direct), Some(PathKindView::Relay), None)
    );
}

#[test]
fn unnamed_relay_shows_as_relay() {
    // given: the receiving side of a relayed link: the relay is not known by name
    let path = NetPath::Relayed(RelayRef::Unnamed);
    // when
    let view = PathKindView::from_net(&path);
    // then
    assert_eq!(view, Some(PathKindView::Relay));
}
