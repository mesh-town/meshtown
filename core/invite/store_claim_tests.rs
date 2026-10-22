//! Tests of redemption claims (`store.rs`): the winner rule `settle` and the selection of live rivals
//! `live_rivals`. Writing claims into the document and the race of two admitting nodes are covered by end-to-end tests.

use super::*;
use crate::net::NodeSecret;
use bytes::Bytes;

const NOW: u64 = 1_790_000_000;

fn id(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

/// Two different ids: first the smaller by `as_bytes()`, then the larger.
fn ordered_pair() -> (PeerId, PeerId) {
    let (a, b) = (id(1), id(2));
    match a.as_bytes() < b.as_bytes() {
        true => (a, b),
        false => (b, a),
    }
}

fn claim_entry(author: PeerId, written_unix: u64, live: bool) -> ReplicaEntry {
    let value = match live {
        true => Bytes::from_static(b"x"),
        false => Bytes::new(),
    };
    ReplicaEntry { author, key: b"invite-claim/h".to_vec(), value, ts_micros: written_unix * 1_000_000 }
}

mod settle_by_peer_id {
    use super::*;

    #[test]
    fn settle_without_rivals_wins() {
        assert_eq!(settle(id(1), &[]), Settle::Win);
    }

    #[test]
    fn settle_yields_to_a_smaller_id() {
        let (small, large) = ordered_pair();
        assert_eq!(settle(large, &[small]), Settle::Yield);
    }

    #[test]
    fn settle_wins_over_a_larger_id() {
        let (small, large) = ordered_pair();
        assert_eq!(settle(small, &[large]), Settle::Win);
    }
}

mod live_rivals_filter {
    use super::*;

    #[test]
    fn live_rivals_skip_own_tombstones_and_stale() {
        // given: our own claim, a foreign tombstone, a foreign one older than 30 s and a fresh foreign one
        let (me, fresh) = ordered_pair();
        let (tomb, stale) = (id(3), id(4));
        let entries = [
            claim_entry(me, NOW, true),
            claim_entry(tomb, NOW, false),
            claim_entry(stale, NOW - CLAIM_STALE_SECS - 1, true),
            claim_entry(fresh, NOW - 1, true),
        ];
        // when
        let got = live_rivals(&entries, me, NOW);
        // then: only the fresh foreign one
        assert_eq!(got, vec![fresh]);
    }

    #[test]
    fn claim_from_the_future_is_live() {
        let (me, other) = ordered_pair();
        let entries = [claim_entry(other, NOW + 100, true)];
        assert_eq!(live_rivals(&entries, me, NOW), vec![other]);
    }
}
