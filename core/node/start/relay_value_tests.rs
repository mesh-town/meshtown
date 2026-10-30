//! The value of a relay advert: a 32-byte relay key, then an optional IPv4 address and UDP port. A reader takes
//! the key from any value of 32 bytes or more, the address only from a complete tail, and ignores what follows.

use super::*;
use crate::members::access::view::AccessView;
use crate::transport::net::contract::secret;
use std::net::Ipv4Addr;

const KEY: u8 = 7;

mod reading {
    use super::*;

    #[test]
    fn empty_value_is_no_advert() {
        // when
        let got = decode_relay_value(&[]);
        // then
        assert_eq!(got, None);
    }

    #[test]
    fn thirty_one_bytes_is_no_advert() {
        // when
        let got = decode_relay_value(&value_of(31));
        // then
        assert_eq!(got, None, "31 bytes were taken for a key");
    }

    #[test]
    fn bare_key_has_no_address() {
        // when
        let got = decode_relay_value(&value_of(32));
        // then
        assert_eq!(got, Some(([KEY; 32], None)));
    }

    #[test]
    fn short_tail_has_no_address() {
        for len in [33, 37] {
            // when
            let got = decode_relay_value(&value_of(len));
            // then
            assert_eq!(got, Some(([KEY; 32], None)), "a tail of {} bytes", len - 32);
        }
    }

    #[test]
    fn full_value_gives_the_address() {
        // given: 203.0.113.9, port 0x1F90 = 8080
        let mut value = vec![KEY; 32];
        value.extend_from_slice(&[203, 0, 113, 9, 0x1F, 0x90]);
        // when
        let got = decode_relay_value(&value);
        // then
        assert_eq!(got, Some(([KEY; 32], Some(addr([203, 0, 113, 9], 8080)))));
    }

    #[test]
    fn longer_tail_is_ignored() {
        // given: 40 bytes, the last two are room for a future field
        let mut value = vec![KEY; 32];
        value.extend_from_slice(&[203, 0, 113, 9, 0x1F, 0x90, 0xAA, 0xBB]);
        // when
        let got = decode_relay_value(&value);
        // then
        assert_eq!(got, Some(([KEY; 32], Some(addr([203, 0, 113, 9], 8080)))));
    }

    #[test]
    fn unspecified_address_is_dropped() {
        // given: 0.0.0.0:8080
        let value = tailed([0, 0, 0, 0], 8080);
        // when
        let got = decode_relay_value(&value);
        // then: the key is still good
        assert_eq!(got, Some(([KEY; 32], None)));
    }

    #[test]
    fn zero_port_is_dropped() {
        // given: 203.0.113.9:0
        let value = tailed([203, 0, 113, 9], 0);
        // when
        let got = decode_relay_value(&value);
        // then: the key is still good
        assert_eq!(got, Some(([KEY; 32], None)));
    }
}

mod writing {
    use super::*;

    #[test]
    fn written_value_round_trips() {
        // given
        let advert = advert_at([198, 51, 100, 4], 40_000);
        // when
        let value = encode_relay_value(&advert);
        // then
        assert_eq!(value.len(), 38);
        assert_eq!(
            decode_relay_value(&value),
            Some((advert.key, Some(advert.address)))
        );
    }

    #[test]
    fn old_reader_sees_the_key_first() {
        // given
        let advert = advert_at([198, 51, 100, 4], 40_000);
        // when: a reader that knows only the key takes the first 32 bytes
        let value = encode_relay_value(&advert);
        // then
        assert_eq!(&value[..32], &advert.key[..]);
    }
}

mod candidate_of_an_advert {
    use super::*;

    #[test]
    fn advert_with_an_address_gives_a_candidate_with_it() {
        // given: member 5 is live and advertises a relay with an address, member 6 only a key
        let members = Members::with_access(AccessView::open());
        members.add_verified_conn(peer(5));
        members.add_verified_conn(peer(6));
        let advert = advert_at([198, 51, 100, 4], 40_000);
        let adverts: HashMap<PeerId, Bytes> = [
            (peer(5), encode_relay_value(&advert)),
            (peer(6), Bytes::from(value_of(32))),
        ]
        .into_iter()
        .collect();
        // when
        let got = relay_candidates(&members, &adverts, peer(1), peer(2));
        // then
        let expected = |m: u8, relay, address| RelayCandidate {
            member: peer(m),
            relay,
            address,
        };
        assert_eq!(
            got,
            vec![
                expected(5, advert.key, Some(advert.address)),
                expected(6, [KEY; 32], None),
            ]
        );
    }
}

// --- helpers ---

fn peer(seed: u8) -> PeerId {
    secret(seed).peer_id()
}

fn addr(octets: [u8; 4], port: u16) -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::from(octets), port)
}

/// An advert of the relay `[KEY; 32]` at `octets:port`.
fn advert_at(octets: [u8; 4], port: u16) -> RelayAdvert {
    RelayAdvert {
        key: [KEY; 32],
        address: addr(octets, port),
    }
}

/// A value of `len` bytes, all `KEY`.
fn value_of(len: usize) -> Vec<u8> {
    vec![KEY; len]
}

/// The key followed by a complete tail with the given address and port.
fn tailed(octets: [u8; 4], port: u16) -> Vec<u8> {
    let mut value = value_of(32);
    value.extend_from_slice(&octets);
    value.extend_from_slice(&port.to_be_bytes());
    value
}
