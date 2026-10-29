//! The options of a dial through the relay of a candidate: the relay key, its address when the candidate has one,
//! and, in tests, a veto of the hole punch.

use super::dialer::relay_connect_opts;
use crate::transport::net::contract::pid;
use crate::transport::net_hyper::relay::RelayCandidate;
use std::net::{Ipv4Addr, SocketAddrV4};
use townjump_dht::{RelayAddress, RelayThrough};

const RELAY: [u8; 32] = [9; 32];

#[test]
fn key_is_the_relay_through() {
    // given
    let cand = candidate(None);
    // when
    let opts = relay_connect_opts(&cand, false);
    // then
    assert!(
        matches!(opts.relay_through, Some(RelayThrough::Key(k)) if k == RELAY),
        "relay_through is {:?}",
        opts.relay_through
    );
    assert!(opts.reusable_socket, "the dial must reuse the socket");
}

#[test]
fn address_goes_to_relay_addresses() {
    // given
    let address = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 4), 40_000);
    let cand = candidate(Some(address));
    // when
    let opts = relay_connect_opts(&cand, false);
    // then
    assert_eq!(
        opts.relay_addresses,
        vec![RelayAddress {
            public_key: RELAY,
            addr: address
        }]
    );
}

#[test]
fn no_address_means_no_relay_addresses() {
    // given: the advert of an older relay carries the key alone
    let cand = candidate(None);
    // when
    let opts = relay_connect_opts(&cand, false);
    // then
    assert!(
        opts.relay_addresses.is_empty(),
        "got {:?}",
        opts.relay_addresses
    );
}

#[test]
fn vetoed_opts_forbid_punching_and_local_links() {
    // given
    let cand = candidate(None);
    // when
    let opts = relay_connect_opts(&cand, true);
    // then: a filter is set (it refuses every punch) and the local link is off
    assert!(opts.holepunch.is_some(), "no hole punch filter");
    assert!(!opts.local_connection, "a local link stays allowed");
}

#[test]
fn plain_opts_keep_local_links() {
    // given
    let cand = candidate(None);
    // when
    let opts = relay_connect_opts(&cand, false);
    // then
    assert!(opts.holepunch.is_none(), "the punch is vetoed");
    assert!(opts.local_connection, "local links are off");
}

// --- helpers ---

fn candidate(address: Option<SocketAddrV4>) -> RelayCandidate {
    RelayCandidate {
        member: pid(1),
        relay: RELAY,
        address,
    }
}
