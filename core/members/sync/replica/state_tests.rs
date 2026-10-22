//! Tests for `state.rs`: merging, the author's clock, selections and summaries.

use super::*;
use crate::members::sync::replica::entry::seal;
use crate::transport::net::id::NodeSecret;
use crate::transport::net::replica::live;

const T: u64 = 1_000_000_000;

fn secret(n: u8) -> NodeSecret {
    NodeSecret::from_bytes(&[n; 32])
}

fn rid() -> ReplicaId {
    ReplicaId([1; 32])
}

fn state_of(n: u8) -> State {
    State::new(rid(), secret(n).peer_id())
}

fn put(n: u8, key: &[u8], value: &[u8], ts: u64) -> WireEntry {
    seal(&secret(n), &rid(), key, value, ts)
}

mod merge {
    use super::*;

    #[test]
    fn added_replaced_ignored() {
        let mut s = state_of(1);
        assert!(s.merge(put(2, b"k", b"a", 10)), "new");
        assert!(s.merge(put(2, b"k", b"b", 11)), "newer of the same pair");
        let before = s.entries();
        assert!(!s.merge(put(2, b"k", b"a", 10)), "older");
        assert!(!s.merge(put(2, b"k", b"b", 11)), "the same");
        assert_eq!(s.entries(), before, "the map did not change");
    }

    #[test]
    fn tombstone_hides_from_live() {
        let mut s = state_of(1);
        s.merge(put(2, b"k", b"a", 10));
        s.merge(put(2, b"k", b"", 11));
        assert!(live(s.scan(b"")).is_empty());
        assert_eq!(s.scan(b"").len(), 1, "tombstone in scan");
        assert!(s.get(secret(2).peer_id(), b"k").is_none());
    }
}

mod clock {
    use super::*;

    #[test]
    fn next_ts_monotone_when_clock_goes_back() {
        let mut s = state_of(1);
        s.merge(put(1, b"k", b"a", T));
        assert_eq!(s.next_ts_micros(T - 1_000_000), T + 1);
        assert_eq!(s.next_ts_micros(T + 50), T + 50);
    }

    #[test]
    fn restored_own_entry_raises_max() {
        let mut s = state_of(1);
        s.merge(put(1, b"k", b"a", T + 5));
        assert!(s.next_ts_micros(T - 1) > T + 5);
    }

    #[test]
    fn foreign_entry_does_not_raise_max() {
        let mut s = state_of(1);
        s.merge(put(2, b"k", b"a", T + 5));
        assert_eq!(s.next_ts_micros(T), T);
    }
}

mod scan {
    use super::*;

    #[test]
    fn author_then_key() {
        let mut s = state_of(1);
        for (n, k) in [(3u8, &b"a"[..]), (2, b"z"), (2, b"b"), (3, b"x/1")] {
            s.merge(put(n, k, b"v", 1));
        }
        let got: Vec<(PeerId, Vec<u8>)> =
            s.scan(b"").into_iter().map(|e| (e.author, e.key)).collect();
        let mut want = got.clone();
        want.sort();
        assert_eq!(got, want);
        assert_eq!(s.scan(b"x/").len(), 1, "prefix");
    }
}

mod latest {
    use super::*;

    #[test]
    fn tie_breaks_by_author() {
        let mut s = state_of(1);
        s.merge(put(2, b"k", b"two", 5));
        s.merge(put(3, b"k", b"three", 5));
        let (a2, a3) = (secret(2).peer_id(), secret(3).peer_id());
        let want = if a3 > a2 { a3 } else { a2 };
        let got = s.latest_per_key(b"k");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].author, want);
    }

    #[test]
    fn newer_ts_beats_author() {
        let mut s = state_of(1);
        s.merge(put(2, b"k", b"two", 6));
        s.merge(put(3, b"k", b"three", 5));
        assert_eq!(s.latest_per_key(b"k")[0].author, secret(2).peer_id());
    }
}

mod summary {
    use super::*;

    #[test]
    fn equal_states_equal_sums() {
        let (mut a, mut b) = (state_of(1), state_of(2));
        for s in [&mut a, &mut b] {
            s.merge(put(3, b"k", b"v", 4));
            s.merge(put(4, b"j", b"v", 5));
        }
        assert_eq!(a.summary(), b.summary());
        assert_eq!(a.summary().len(), 2);
        b.merge(put(4, b"j", b"w", 5));
        assert_ne!(
            a.summary(),
            b.summary(),
            "equal ts, a different value is visible"
        );
    }
}

mod missing {
    use super::*;

    #[test]
    fn empty_theirs_gets_all() {
        let mut a = state_of(1);
        a.merge(put(2, b"k", b"v", 1));
        a.merge(put(3, b"k", b"v", 1));
        assert_eq!(a.missing_for(&[]).len(), 2);
    }

    #[test]
    fn only_differing_author_sent() {
        let (mut a, mut b) = (state_of(1), state_of(2));
        for s in [&mut a, &mut b] {
            s.merge(put(3, b"k", b"v", 1));
        }
        a.merge(put(4, b"k", b"v", 1));
        let got = a.missing_for(&b.summary());
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].author, *secret(4).peer_id().as_bytes());
    }

    #[test]
    fn gap_below_max_is_seen() {
        let (mut a, mut b) = (state_of(1), state_of(2));
        let old = put(3, b"old", b"v", 5);
        let new = put(3, b"new", b"v", 9);
        a.merge(old.clone());
        a.merge(new.clone());
        b.merge(new);
        assert_eq!(a.summary()[0].max_ts_micros, b.summary()[0].max_ts_micros);
        assert!(a.missing_for(&b.summary()).contains(&old));
    }
}

mod load {
    use super::*;

    #[test]
    fn bad_signature_dropped_and_counted() {
        let mut bad = put(2, b"bad", b"v", 1);
        bad.value = b"x".to_vec();
        let good = put(2, b"good", b"v", 1);
        let (s, rejected) = State::from_entries(rid(), secret(1).peer_id(), vec![bad, good]);
        assert_eq!(rejected, 1);
        assert_eq!(s.entries().len(), 1);
        assert_eq!(s.scan(b"good").len(), 1);
    }
}
