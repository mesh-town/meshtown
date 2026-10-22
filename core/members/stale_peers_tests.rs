//! Tests of a member's age and of dropping the departed (`stale_peers.rs`).

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::transport::net::{NodeSecret, PeerId, ReplicaEntry};
use bytes::Bytes;

use super::*;
use crate::node::telemetry::TelemetryView;
use crate::site_checks::reachability::{CheckRecord, CheckVerdict, CHECK_RECORD_VERSION};

const T: u64 = 1_000_000_000;
const D: u64 = 86_400;

fn id(seed: u8) -> PeerId {
    NodeSecret::from_bytes(&[seed; 32]).peer_id()
}

struct Rig {
    members: HashSet<PeerId>,
    tel: TelemetryView,
    checks: HashMap<PeerId, u64>,
    cached: HashMap<PeerId, u64>,
    doc: HashMap<PeerId, u64>,
}

impl Rig {
    fn new() -> Rig {
        Rig {
            members: HashSet::new(),
            tel: TelemetryView::default(),
            checks: HashMap::new(),
            cached: HashMap::new(),
            doc: HashMap::new(),
        }
    }

    fn age(&self, x: PeerId) -> Option<u64> {
        let ev = Evidence {
            members: &self.members,
            telemetry: &self.tel,
            now: Instant::now(),
            now_unix: T,
            check_seen_unix: &self.checks,
            cache_seen_unix: &self.cached,
            doc_seen_unix: &self.doc,
        };
        age_sec(&ev, x)
    }
}

mod age {
    use super::*;

    #[test]
    fn member_is_zero_even_with_old_cache() {
        let x = id(2);
        let mut r = Rig::new();
        r.members.insert(x);
        r.cached.insert(x, T - 8 * D);
        assert_eq!(r.age(x), Some(0));
    }

    #[test]
    fn takes_minimum_of_sources() {
        let x = id(2);
        let mut r = Rig::new();
        r.cached.insert(x, T - 8 * D);
        r.doc.insert(x, T - D);
        assert_eq!(r.age(x), Some(D));
    }

    #[test]
    fn future_cache_gives_zero() {
        let x = id(2);
        let mut r = Rig::new();
        r.cached.insert(x, T + 5 * D);
        assert_eq!(r.age(x), Some(0));
    }

    #[test]
    fn no_sources_is_none() {
        assert_eq!(Rig::new().age(id(2)), None);
    }
}

mod gone {
    use super::*;

    #[test]
    fn eight_days_is_gone() {
        assert!(is_gone(Some(8 * D)));
    }

    #[test]
    fn six_days_stays() {
        assert!(!is_gone(Some(6 * D)));
    }

    #[test]
    fn exactly_seven_days_stays() {
        assert!(!is_gone(Some(7 * D)));
    }

    #[test]
    fn seven_days_and_one_sec_is_gone() {
        assert!(is_gone(Some(7 * D + 1)));
    }

    #[test]
    fn none_is_gone() {
        assert!(is_gone(None));
    }
}

mod doc_seen {
    use super::*;

    fn entry(x: PeerId, ts_micros: u64) -> ReplicaEntry {
        ReplicaEntry {
            author: x,
            key: b"node/test/".to_vec(),
            value: Bytes::from_static(b"v"),
            ts_micros,
        }
    }

    #[test]
    fn max_per_author_in_seconds() {
        let x = id(2);
        let got = fold_doc_seen(
            &[
                entry(x, 5_000_000_000_000_000),
                entry(x, 6_000_000_000_000_000),
            ],
            u64::MAX,
        );
        assert_eq!(got.get(&x), Some(&6_000_000_000));
    }

    #[test]
    fn future_timestamp_clamped_to_now() {
        let x = id(2);
        let got = fold_doc_seen(&[entry(x, (T + 100 * D) * 1_000_000)], T);
        assert_eq!(got.get(&x), Some(&T));
    }
}

mod freshest_checks {
    use super::*;

    #[test]
    fn newest_record_of_author_wins() {
        // given: the author checked two domains of the list — at 100 and at 300
        let x = id(2);
        let views = [check("a.com", x, Some(100)), check("b.com", x, Some(300))];
        // when: we compute the freshness of the checks
        let got = latest_check_unix_by_author(&views, &listed(&["a.com", "b.com"]));
        // then: the author gets the greatest time
        assert_eq!(got.get(&x), Some(&300));
    }

    #[test]
    fn authors_are_kept_apart() {
        // given: two authors checked one domain at different times
        let (x, y) = (id(2), id(3));
        let views = [check("a.com", x, Some(100)), check("a.com", y, Some(200))];
        // when: we compute the freshness of the checks
        let got = latest_check_unix_by_author(&views, &listed(&["a.com"]));
        // then: each has their own time, the key is the node id
        assert_eq!((got.get(&x), got.get(&y)), (Some(&100), Some(&200)));
    }

    #[test]
    fn record_without_content_is_skipped() {
        // given: an author's entry without parsed content (not arrived, a tombstone, a foreign format)
        let x = id(2);
        let views = [check("a.com", x, None)];
        // when: we compute the freshness of the checks
        let got = latest_check_unix_by_author(&views, &listed(&["a.com"]));
        // then: the author is absent from the result
        assert!(got.is_empty(), "unexpected: {got:?}");
    }

    #[test]
    fn domain_outside_list_is_skipped() {
        // given: an author checked a domain that is not in the node's list
        let x = id(2);
        let views = [check("z.com", x, Some(500))];
        // when: we compute the freshness of the checks
        let got = latest_check_unix_by_author(&views, &listed(&["a.com"]));
        // then: the author is absent from the result
        assert!(got.is_empty(), "unexpected: {got:?}");
    }
}

// --- helpers ---

fn listed(domains: &[&str]) -> Vec<String> {
    domains.iter().map(|d| d.to_string()).collect()
}

/// A domain check entry from `author`; `at` is the time of the check itself, `None` — no content.
fn check(domain: &str, author: PeerId, at: Option<u64>) -> CheckView {
    CheckView {
        domain: domain.to_string(),
        author,
        entry_ts_micros: 0,
        record: at.map(|at| CheckRecord {
            v: CHECK_RECORD_VERSION,
            verdict: CheckVerdict::Ok,
            dns_ms: 1,
            tcp_ms: 2,
            tls_ms: 3,
            http_ms: 4,
            status: 200,
            bytes: 100,
            stalled: false,
            at,
            reason: "ok".to_string(),
        }),
    }
}
