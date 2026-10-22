//! Tests for `net/addr.rs`: `PeerAddr::new` and `PeerAddr::merge` (skeleton — `tests.md`).

use super::*;
use crate::transport::net::id::{NodeSecret, PeerId};
use std::net::SocketAddr;

mod new {
    use super::*;

    #[test]
    fn keeps_the_given_id() {
        assert_eq!(PeerAddr::new(id_of(1)).id, id_of(1));
    }

    #[test]
    fn starts_without_hints() {
        assert!(
            PeerAddr::new(id_of(1)).hints.is_empty(),
            "a new address has an empty hint list"
        );
    }
}

mod merge {
    use super::*;

    mod same_id {
        use super::*;

        #[test]
        fn overlapping_hints_give_the_union_with_own_first() {
            // given: hints [a, b] and [b, c] of one id
            let (mine, other) = ([ip_a(), ip_b()], [ip_b(), ip_c()]);
            // when: we merge
            let hints = merged(&mine, &other);
            // then: [a, b, c], no repeats
            assert_eq!(hints, vec![ip_a(), ip_b(), ip_c()]);
        }

        #[test]
        fn own_hints_stay_first_whatever_the_order_in_other() {
            let hints = merged(&[ip_b(), ip_c()], &[ip_a(), ip_b()]);
            assert_eq!(
                hints,
                vec![ip_b(), ip_c(), ip_a()],
                "own first, new foreign ones at the end"
            );
        }

        #[test]
        fn new_hints_of_other_keep_their_order() {
            let hints = merged(&[ip_a()], &[ip_c(), ip_b()]);
            assert_eq!(hints, vec![ip_a(), ip_c(), ip_b()]);
        }

        #[test]
        fn merging_an_identical_list_changes_nothing() {
            let hints = merged(&[ip_a(), ip_b()], &[ip_a(), ip_b()]);
            assert_eq!(hints, vec![ip_a(), ip_b()]);
        }

        #[test]
        fn merging_twice_gives_the_same_as_merging_once() {
            let mut mine = addr(id_of(1), &[ip_a(), ip_b()]);
            let other = addr(id_of(1), &[ip_b(), ip_c()]);
            mine.merge(&other);
            let once = mine.clone();
            mine.merge(&other);
            assert_eq!(mine, once, "a repeated merge adds nothing");
        }

        #[test]
        fn a_hint_repeated_inside_other_is_added_once() {
            let hints = merged(&[ip_a()], &[ip_c(), ip_c()]);
            assert_eq!(hints, vec![ip_a(), ip_c()], "union without repeats");
        }

        #[test]
        fn empty_other_changes_nothing() {
            assert_eq!(merged(&[ip_a(), ip_b()], &[]), vec![ip_a(), ip_b()]);
        }

        #[test]
        fn into_an_empty_list_takes_the_hints_of_other_in_order() {
            assert_eq!(merged(&[], &[ip_b(), ip_a()]), vec![ip_b(), ip_a()]);
        }

        #[test]
        fn hints_of_different_kinds_with_alike_payload_are_kept_apart() {
            // given: hints that are only partly equal (variant, type, kind, bytes)
            let (peer_relay, url_relay) = (relay_peer(2), relay_url("relay.example"));
            let mine = [opaque(1, &[9]), peer_relay.clone()];
            let other = [
                opaque(2, &[9]),
                opaque(1, &[8]),
                url_relay.clone(),
                opaque(1, &[9]),
            ];
            // when: we merge
            let hints = merged(&mine, &other);
            // then: those that differ in value are different hints, an equal one is one
            let want = vec![
                opaque(1, &[9]),
                peer_relay,
                opaque(2, &[9]),
                opaque(1, &[8]),
                url_relay,
            ];
            assert_eq!(hints, want);
        }

        #[test]
        fn keeps_the_id() {
            let mut mine = addr(id_of(1), &[ip_a()]);
            mine.merge(&addr(id_of(1), &[ip_b()]));
            assert_eq!(mine.id, id_of(1));
        }
    }

    mod other_id {
        use super::*;

        #[test]
        fn leaves_the_hints_unchanged() {
            // given: a foreign id with other hints
            let mut mine = addr(id_of(1), &[ip_a(), ip_b()]);
            let stranger = addr(id_of(2), &[ip_b(), ip_c()]);
            // when: we merge
            mine.merge(&stranger);
            // then: nothing changed
            assert_eq!(
                mine.hints,
                vec![ip_a(), ip_b()],
                "foreign hints are not accepted"
            );
        }

        #[test]
        fn leaves_the_whole_address_unchanged() {
            let mut mine = addr(id_of(1), &[ip_a()]);
            let before = mine.clone();
            mine.merge(&addr(id_of(2), &[ip_c()]));
            assert_eq!(mine, before, "neither the id nor the hints change");
        }
    }
}

// --- setup and helpers ---

fn id_of(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

fn addr(id: PeerId, hints: &[AddrHint]) -> PeerAddr {
    let mut addr = PeerAddr::new(id);
    addr.hints = hints.to_vec();
    addr
}

/// Merge of `other` into `mine` for the same id; the result is the hint list.
fn merged(mine: &[AddrHint], other: &[AddrHint]) -> Vec<AddrHint> {
    let mut mine = addr(id_of(1), mine);
    mine.merge(&addr(id_of(1), other));
    mine.hints
}

fn ip(text: &str) -> AddrHint {
    AddrHint::Ip(
        text.parse::<SocketAddr>()
            .expect("the address in the test is valid"),
    )
}

fn ip_a() -> AddrHint {
    ip("192.0.2.1:4000")
}

fn ip_b() -> AddrHint {
    ip("198.51.100.7:4001")
}

fn ip_c() -> AddrHint {
    ip("[2001:db8::1]:4002")
}

fn relay_peer(n: u8) -> AddrHint {
    AddrHint::Relay(RelayRef::Peer(id_of(n)))
}

fn relay_url(url: &str) -> AddrHint {
    AddrHint::Relay(RelayRef::Url(url.to_string()))
}

fn opaque(kind: u64, bytes: &[u8]) -> AddrHint {
    AddrHint::Opaque {
        kind,
        bytes: bytes.to_vec(),
    }
}
