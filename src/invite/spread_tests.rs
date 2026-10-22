//! Tests of collecting holders: whom to ask, the order of the string's entries, the reach flag, the wait limit.

use super::*;
use crate::invite::{Reach, MAX_INVITE_PEERS};
use crate::net::NodeSecret;

fn id(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

#[test]
fn candidates_skip_me_and_stop_at_four() {
    // given: seven members, myself among them
    let members: Vec<PeerId> = (1..=7).map(id).collect();
    // when
    let got = candidates(&members, id(3));
    // then: myself is absent, no more than HOLD_ASK_MAX, order by key
    assert_eq!(got.len(), HOLD_ASK_MAX);
    assert!(!got.contains(&id(3)));
    assert!(got.windows(2).all(|w| w[0].as_bytes() < w[1].as_bytes()));
}

#[test]
fn ordered_puts_holders_then_me_then_others_without_repeats() {
    // given: two holders, myself, the rest (one repeats a holder)
    let me = PeerAddr::new(id(9));
    let list = ordered(&[id(1), id(2)], &me, &[id(2), id(3)]);
    // when: the order is turned into keys
    let ids: Vec<PeerId> = list.iter().map(|a| a.id).collect();
    // then: holders, the creator, the rest; the repeat stays for build_as, which will drop it
    assert_eq!(ids, vec![id(1), id(2), id(9), id(2), id(3)]);
    assert!(ids.len() <= MAX_INVITE_PEERS);
}

#[test]
fn rest_puts_connected_first_then_offline_listed() {
    // given: c is connected, b and d are listed but offline, a is me
    let admitted: BTreeSet<PeerId> = [id(2), id(3), id(4), id(1)].into_iter().collect();
    // when
    let got = rest(&[id(3)], &admitted, id(1));
    // then: connected first, then the others by key, myself nowhere
    let mut offline = vec![id(2), id(4)];
    offline.sort_by_key(|p| *p.as_bytes());
    assert_eq!(got, [vec![id(3)], offline].concat());
}

#[test]
fn rest_without_a_list_is_the_connected_only() {
    let got = rest(&[id(3), id(1), id(3)], &BTreeSet::new(), id(1));
    assert_eq!(got, vec![id(3)]);
}

#[test]
fn reach_of_0_1_2() {
    assert_eq!(Reach::of(0), Reach::IssuerOnly);
    assert_eq!(Reach::of(1), Reach::IssuerAndOne);
    assert_eq!(Reach::of(2), Reach::Spread);
    assert_eq!(Reach::of(0).as_str(), "issuer_only");
    assert_eq!(Reach::of(1).as_str(), "issuer_and_one");
    assert_eq!(Reach::of(2).as_str(), "spread");
}

#[test]
fn hold_wait_fits_the_tray_timeout() {
    assert!(HOLD_WAIT < Duration::from_secs(2));
}
