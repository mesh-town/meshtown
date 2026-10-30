//! What a running relay can offer right now: its address when it is open, its key alone when it is behind a
//! consistent NAT, nothing to wait for while unknown, and "unfit" when the NAT maps every peer to another port.

use super::*;
use std::net::{Ipv4Addr, SocketAddrV4};
use townjump_dht::DhtError;
use townjump_dht_peeroxide::{NatKind, RelayReachability};

const KEY: [u8; 32] = [7; 32];

mod offer_of {
    use super::*;

    #[test]
    fn open_with_an_address_serves_it() {
        // given
        let a = addr(40_000);
        // when
        let offer = offer_of(KEY, NatKind::Open, Some(a));
        // then
        assert_eq!(offer, Offer::Serve(advert(Some(a))));
    }

    #[test]
    fn open_without_an_address_waits() {
        // when
        let offer = offer_of(KEY, NatKind::Open, None);
        // then
        assert_eq!(
            offer,
            Offer::Wait,
            "an open relay without an address served"
        );
    }

    #[test]
    fn consistent_serves_the_bare_key() {
        for address in [None, Some(addr(40_000))] {
            // when
            let offer = offer_of(KEY, NatKind::Consistent, address);
            // then
            assert_eq!(
                offer,
                Offer::Serve(advert(None)),
                "a consistent NAT with address {address:?}"
            );
        }
    }

    #[test]
    fn random_is_unfit() {
        for address in [None, Some(addr(40_000))] {
            // when
            let offer = offer_of(KEY, NatKind::Random, address);
            // then
            assert_eq!(offer, Offer::Unfit, "a random NAT with address {address:?}");
        }
    }

    #[test]
    fn unknown_waits() {
        for address in [None, Some(addr(40_000))] {
            // when
            let offer = offer_of(KEY, NatKind::Unknown, address);
            // then
            assert_eq!(
                offer,
                Offer::Wait,
                "an unknown NAT with address {address:?}"
            );
        }
    }
}

mod offer_of_reading {
    use super::*;

    #[test]
    fn failed_reading_waits() {
        // when
        let offer = offer_of_reading(KEY, Err(DhtError::Destroyed));
        // then
        assert_eq!(offer, Offer::Wait);
    }
    #[test]
    fn reading_consistent_serves_the_bare_key() {
        // when
        let offer = offer_of_reading(KEY, Ok(reading(NatKind::Consistent, None)));
        // then
        assert_eq!(offer, Offer::Serve(advert(None)));
    }

    #[test]
    fn reading_open_with_an_address_serves_it() {
        // given
        let a = addr(40_000);
        // when
        let offer = offer_of_reading(KEY, Ok(reading(NatKind::Open, Some(a))));
        // then
        assert_eq!(offer, Offer::Serve(advert(Some(a))));
    }

    #[test]
    fn reading_random_is_unfit() {
        // when
        let offer = offer_of_reading(KEY, Ok(reading(NatKind::Random, None)));
        // then
        assert_eq!(offer, Offer::Unfit);
    }
}

// --- helpers ---

fn addr(port: u16) -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::new(203, 0, 113, 9), port)
}

/// The advert of the relay `KEY` at `address`.
fn advert(address: Option<SocketAddrV4>) -> RelayAdvert {
    RelayAdvert { key: KEY, address }
}

fn reading(firewall: NatKind, public_address: Option<SocketAddrV4>) -> RelayReachability {
    RelayReachability {
        local_port: 49_737,
        public_address,
        firewall,
    }
}
