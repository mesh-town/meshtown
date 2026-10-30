//! Relay adverts without an address: a relay behind a NAT is written as its bare 32-byte key, read back as a
//! relay reached by key, and offered after the relays that can be reached at an address.

use super::*;
use crate::members::access::tally::Ledger;
use crate::members::access::view::{AccessView, Verdicts};
use crate::transport::net::contract::secret;
use std::collections::BTreeSet;

const KEY: u8 = 7;
const ME: u8 = 1;
const TARGET: u8 = 2;

mod writing {
    use super::*;

    #[test]
    fn key_only_advert_is_the_bare_key() {
        // given
        let advert = RelayAdvert {
            key: [KEY; 32],
            address: None,
        };
        // when
        let value = encode_relay_value(&advert);
        // then
        assert_eq!(value.len(), 32, "a tail was written for an absent address");
        assert_eq!(&value[..], &advert.key[..]);
    }

    #[test]
    fn key_only_value_reads_back_without_an_address() {
        // given
        let advert = RelayAdvert {
            key: [KEY; 32],
            address: None,
        };
        // when
        let got = decode_relay_value(&encode_relay_value(&advert));
        // then
        assert_eq!(got, Some(([KEY; 32], None)));
    }
}

mod candidates {
    use super::*;

    #[test]
    fn addressed_candidates_come_before_key_only_ones() {
        // given: m1 and m3 are reached by key, m2 at an address
        let members = members_admitting(&[5, 6, 7], &[5, 6, 7]);
        let adverts = adverts(&[(5, key_only()), (6, addressed()), (7, key_only())]);
        // when
        let got = relay_candidates(&members, &adverts, peer(ME), peer(TARGET));
        // then: the addressed one first, the others by member
        let mut by_key: Vec<PeerId> = vec![peer(5), peer(7)];
        by_key.sort();
        let expected: Vec<PeerId> = [vec![peer(6)], by_key].concat();
        let order: Vec<PeerId> = got.iter().map(|c| c.member).collect();
        assert_eq!(order, expected);
        assert_eq!(got[0].address, Some(addr()));
        assert_eq!(got[1].address, None);
        assert_eq!(got[2].address, None);
    }

    #[test]
    fn unadmitted_key_only_author_is_skipped() {
        // given: 5 holds a verified connection but the list does not admit it
        let members = members_admitting(&[], &[5]);
        let adverts = adverts(&[(5, key_only())]);
        // when
        let got = relay_candidates(&members, &adverts, peer(ME), peer(TARGET));
        // then
        assert!(got.is_empty(), "an unadmitted author was offered: {got:?}");
    }
}

// --- helpers ---

fn peer(seed: u8) -> PeerId {
    secret(seed).peer_id()
}

fn addr() -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 4), 40_000)
}

fn key_only() -> RelayAdvert {
    RelayAdvert {
        key: [KEY; 32],
        address: None,
    }
}

fn addressed() -> RelayAdvert {
    RelayAdvert {
        address: Some(addr()),
        ..key_only()
    }
}

/// The adverts as written by the members `(author seed, advert)`.
fn adverts(list: &[(u8, RelayAdvert)]) -> HashMap<PeerId, Bytes> {
    list.iter()
        .map(|(seed, advert)| (peer(*seed), encode_relay_value(advert)))
        .collect()
}

/// Members where the list admits `admitted` and the verified connections are to `connected`.
fn members_admitting(admitted: &[u8], connected: &[u8]) -> Members {
    let mut ledger = Ledger::empty();
    ledger.admitted = admitted.iter().map(|s| peer(*s)).collect();
    ledger.admitted.insert(peer(ME));
    let view = AccessView::open();
    view.publish(Verdicts {
        me: peer(ME),
        ledger: Arc::new(ledger),
        blocks: BTreeSet::new(),
    });
    let members = Members::with_access(view);
    for seed in connected {
        members.add_verified_conn(peer(*seed));
    }
    members
}
