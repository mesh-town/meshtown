//! `HintBook`: which addresses are kept per peer, in what order they are tried, and what the limits are.

use super::*;
use crate::transport::net::addr::{AddrHint, PeerAddr, RelayRef};
use crate::transport::net::id::{NodeSecret, PeerId};
use std::net::{SocketAddr, SocketAddrV4};

mod note {
    use super::*;

    #[test]
    fn keeps_only_ipv4_ip_hints() {
        // given
        let book = HintBook::default();
        let mixed = PeerAddr {
            id: peer(1),
            hints: vec![
                AddrHint::Ip(v6_sock()),
                AddrHint::Relay(RelayRef::Url("https://relay.example/".into())),
                AddrHint::Opaque {
                    kind: 7,
                    bytes: vec![1, 2, 3],
                },
                AddrHint::Ip(SocketAddr::V4(sock(1))),
            ],
        };
        // when
        book.note(&mixed);
        // then
        assert_eq!(book.targets(peer(1)), vec![sock(1)]);
    }

    #[test]
    fn ignores_an_address_without_usable_hints() {
        // given
        let book = HintBook::default();
        let unusable = PeerAddr {
            id: peer(1),
            hints: vec![AddrHint::Ip(v6_sock())],
        };
        // when
        book.note(&unusable);
        book.note(&PeerAddr::new(peer(1)));
        // then
        assert!(book.targets(peer(1)).is_empty());
    }

    #[test]
    fn keeps_the_two_newest_hints_per_peer() {
        // given
        let book = HintBook::default();
        // when
        for n in 1..=3 {
            book.note(&hinted(peer(1), sock(n)));
        }
        // then
        assert_eq!(book.targets(peer(1)), vec![sock(3), sock(2)]);
    }

    #[test]
    fn moves_a_repeated_hint_to_the_front() {
        // given
        let book = HintBook::default();
        book.note(&hinted(peer(1), sock(1)));
        book.note(&hinted(peer(1), sock(2)));
        // when
        book.note(&hinted(peer(1), sock(1)));
        // then
        assert_eq!(book.targets(peer(1)), vec![sock(1), sock(2)]);
    }

    #[test]
    fn evicts_the_least_recently_touched_peer_over_the_cap() {
        // given
        let book = HintBook::default();
        // when
        for n in 0..=PEERS_CAP {
            book.note(&hinted(peer_n(n), sock(1)));
        }
        // then
        assert!(
            book.targets(peer_n(0)).is_empty(),
            "the oldest peer must be dropped"
        );
        assert_eq!(book.targets(peer_n(1)), vec![sock(1)]);
        assert_eq!(book.targets(peer_n(PEERS_CAP)), vec![sock(1)]);
    }

    #[test]
    fn keeps_a_peer_that_was_noted_again_when_evicting() {
        // given
        let book = HintBook::default();
        for n in 0..PEERS_CAP {
            book.note(&hinted(peer_n(n), sock(1)));
        }
        book.note(&hinted(peer_n(0), sock(1)));
        // when
        book.note(&hinted(peer_n(PEERS_CAP), sock(1)));
        // then
        assert_eq!(book.targets(peer_n(0)), vec![sock(1)]);
        assert!(
            book.targets(peer_n(1)).is_empty(),
            "the peer untouched the longest goes instead"
        );
    }
}

mod targets {
    use super::*;

    #[test]
    fn of_an_unknown_peer_is_empty() {
        assert!(HintBook::default().targets(peer(9)).is_empty());
    }

    #[test]
    fn puts_the_proven_address_first_and_caps_at_two() {
        // given
        let book = HintBook::default();
        book.note(&hinted(peer(1), sock(1)));
        book.note(&hinted(peer(1), sock(2)));
        // when
        book.prove(peer(1), sock(3));
        // then
        assert_eq!(book.targets(peer(1)), vec![sock(3), sock(2)]);
    }

    #[test]
    fn lists_a_proven_address_once_when_it_is_also_a_hint() {
        // given
        let book = HintBook::default();
        book.note(&hinted(peer(1), sock(1)));
        book.note(&hinted(peer(1), sock(2)));
        // when
        book.prove(peer(1), sock(1));
        // then
        assert_eq!(book.targets(peer(1)), vec![sock(1), sock(2)]);
    }
}

mod prove {
    use super::*;

    #[test]
    fn is_reported_by_proven() {
        // given
        let book = HintBook::default();
        // when
        book.prove(peer(1), sock(1));
        // then
        assert_eq!(book.proven(peer(1)), Some(sock(1)));
        assert_eq!(
            book.proven(peer(2)),
            None,
            "another peer has nothing proven"
        );
    }
}

mod disprove {
    use super::*;

    #[test]
    fn forgets_proven_but_keeps_the_hint() {
        // given
        let book = HintBook::default();
        book.note(&hinted(peer(1), sock(1)));
        book.prove(peer(1), sock(1));
        // when
        book.disprove(peer(1), sock(1));
        // then
        assert_eq!(book.proven(peer(1)), None);
        assert_eq!(book.targets(peer(1)), vec![sock(1)]);
    }

    #[test]
    fn of_another_address_keeps_proven() {
        // given
        let book = HintBook::default();
        book.prove(peer(1), sock(1));
        // when
        book.disprove(peer(1), sock(2));
        // then
        assert_eq!(book.proven(peer(1)), Some(sock(1)));
    }
}

const PEERS_CAP: u64 = MAX_HINT_PEERS as u64;

fn sock(n: u8) -> SocketAddrV4 {
    SocketAddrV4::new([203, 0, 113, n].into(), 4000 + u16::from(n))
}

fn v6_sock() -> SocketAddr {
    "[2001:db8::1]:4000".parse().unwrap()
}

fn peer(seed: u8) -> PeerId {
    NodeSecret::from_bytes(&[seed; 32]).peer_id()
}

/// Distinct peers beyond the 256 values of a single seed byte.
fn peer_n(n: u64) -> PeerId {
    let mut seed = [7u8; 32];
    seed[..8].copy_from_slice(&n.to_le_bytes());
    NodeSecret::from_bytes(&seed).peer_id()
}

fn hinted(id: PeerId, at: SocketAddrV4) -> PeerAddr {
    PeerAddr {
        id,
        hints: vec![AddrHint::Ip(SocketAddr::V4(at))],
    }
}
