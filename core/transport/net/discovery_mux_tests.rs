//! Tests for discovery merging (`discovery_mux.rs`): source rank, hint lifetime, broadcast of new ones.
//! Start, stop and publishing are in `discovery_mux_run_tests.rs`.

use crate::transport::net::addr::{AddrHint, PeerAddr};
use crate::transport::net::discovery::Discovery;
use crate::transport::net::discovery_mux::{DiscoveryMux, DiscoveryTiming};
use crate::transport::net::facade::test_support::{addr, hint, pid, within, Script};
use crate::transport::net::id::PeerId;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::error::TryRecvError;
use tokio::time::sleep;

mod lookup {
    use super::*;

    #[test]
    fn unknown_peer_gives_none() {
        let mux = setup(&[]);

        assert!(mux.lookup(&pid(1)).is_none());
    }

    #[test]
    fn merges_hints_of_all_sources_into_one_address() {
        let mux = setup(&["invite", "mainline"]);
        mux.note("invite", addr(1, &[1]));
        mux.note("mainline", addr(1, &[2]));

        let got = mux.lookup(&pid(1)).expect("the address must be found");

        assert_eq!(got.id, pid(1));
        assert_eq!(got.hints, hints(&[1, 2]));
    }
}

mod note {
    use super::*;

    #[test]
    fn empty_note_ignored() {
        let mux = setup(&["invite"]);
        let mut news = mux.subscribe();

        mux.note("invite", addr(1, &[]));

        assert!(mux.lookup(&pid(1)).is_none(), "there must be no record");
        assert_eq!(
            mux.known_peer_count(&pid(9)),
            0,
            "a node without hints is not known"
        );
        assert!(
            matches!(news.try_recv(), Err(TryRecvError::Empty)),
            "there must be no broadcast"
        );
    }

    #[test]
    fn renote_fresh_first() {
        let mux = setup(&["invite"]);
        mux.note("invite", addr(1, &[1, 2]));

        mux.note("invite", addr(1, &[3, 1]));

        assert_eq!(hints_of(&mux, 1), Some(hints(&[3, 1, 2])));
    }

    #[test]
    fn cap_16_drops_oldest() {
        let mux = setup(&["invite"]);
        let first_sixteen: Vec<u16> = (1..=16).collect();
        mux.note("invite", addr(1, &first_sixteen));

        mux.note("invite", addr(1, &[17]));

        let got = hints_of(&mux, 1).expect("the address must be found");
        assert_eq!(got.len(), 16);
        assert_eq!(got[0], hint(17), "a new hint goes first");
        assert!(!got.contains(&hint(16)), "the oldest must drop out");
        assert!(got.contains(&hint(15)));
    }

    #[test]
    fn cap_16_single_note_keeps_first_sixteen() {
        let mux = setup(&["invite"]);
        let seventeen: Vec<u16> = (1..=17).collect();

        mux.note("invite", addr(1, &seventeen));

        let got = hints_of(&mux, 1).expect("the address must be found");
        assert_eq!(got.len(), 16);
        assert!(got.contains(&hint(16)));
        assert!(!got.contains(&hint(17)), "the extra one drops off the tail");
    }

    #[test]
    fn without_subscribers_is_silent() {
        let mux = setup(&["invite"]);

        mux.note("invite", addr(1, &[1]));

        assert_eq!(hints_of(&mux, 1), Some(hints(&[1])));
    }
}

mod source_order {
    use super::*;

    #[test]
    fn order_follows_priority() {
        let mux = setup(&["invite", "mainline"]);
        mux.note("mainline", addr(1, &[1]));

        mux.note("invite", addr(1, &[2]));

        assert_eq!(hints_of(&mux, 1), Some(hints(&[2, 1])));
    }

    #[test]
    fn unlisted_after_listed_by_name() {
        let mut mux = setup(&["invite"]);
        for name in ["zz", "yy", "invite"] {
            mux.add(Script::new(name).shared());
        }
        mux.note("zz", addr(1, &[3]));
        mux.note("yy", addr(1, &[5]));
        mux.note("manual", addr(1, &[4]));
        mux.note("invite", addr(1, &[2]));

        let got = hints_of(&mux, 1);

        assert_eq!(got, Some(hints(&[4, 2, 5, 3])), "manual, invite, yy, zz");
        assert_eq!(mux.source_names(), vec!["invite", "yy", "zz"]);
    }

    #[test]
    fn duplicate_once_at_best() {
        let mux = setup(&["invite", "mainline"]);
        mux.note("mainline", addr(1, &[3, 1]));
        mux.note("invite", addr(1, &[1]));

        let got = hints_of(&mux, 1);

        assert_eq!(
            got,
            Some(hints(&[1, 3])),
            "the shared hint appears once, at the senior source"
        );
    }
}

mod hint_lifetime {
    use super::*;

    #[tokio::test]
    async fn ttl_expires() {
        let mux = setup(&["invite"]);
        mux.note("invite", addr(1, &[1]));
        assert!(
            mux.lookup(&pid(1)).is_some(),
            "control: the hint is alive right away"
        );

        sleep(Duration::from_millis(300)).await;

        assert!(
            mux.lookup(&pid(1)).is_none(),
            "after the lifetime the hint is dead"
        );
    }

    #[tokio::test]
    async fn renote_refreshes() {
        let mux = setup_with(&["invite"], 400);
        mux.note("invite", addr(1, &[1]));
        sleep(Duration::from_millis(250)).await;

        mux.note("invite", addr(1, &[1]));
        sleep(Duration::from_millis(250)).await;

        // 500 ms passed since the first record, more than the 400 ms lifetime; 250 ms since the refresh
        assert_eq!(hints_of(&mux, 1), Some(hints(&[1])));
    }

    #[tokio::test]
    async fn expiry_per_source() {
        let mux = setup(&["invite", "mainline"]);
        mux.note("mainline", addr(1, &[1]));
        sleep(Duration::from_millis(300)).await;

        mux.note("invite", addr(1, &[2]));

        assert_eq!(
            hints_of(&mux, 1),
            Some(hints(&[2])),
            "the old mainline hint is dead"
        );
    }

    #[tokio::test]
    async fn known_peers_except() {
        let mux = setup(&["invite"]);
        mux.note("invite", addr(1, &[1]));
        mux.note("invite", addr(2, &[2]));
        assert_eq!(
            mux.known_peer_count(&pid(1)),
            1,
            "we do not count ourselves"
        );

        sleep(Duration::from_millis(300)).await;

        assert_eq!(
            mux.known_peer_count(&pid(1)),
            0,
            "after the lifetime, nobody"
        );
    }
}

mod announcements {
    use super::*;

    #[test]
    fn broadcast_on_new_only() {
        let mux = setup(&["invite", "mainline"]);
        let mut news = mux.subscribe();

        mux.note("invite", addr(1, &[1]));
        assert_eq!(news.try_recv().ok(), Some(addr(1, &[1])), "new hint");
        mux.note("mainline", addr(1, &[2]));
        assert_eq!(news.try_recv().ok(), Some(addr(1, &[1, 2])), "merge");
        mux.note("mainline", addr(1, &[2]));

        assert!(
            matches!(news.try_recv(), Err(TryRecvError::Empty)),
            "the same thing is not broadcast again"
        );
    }

    #[test]
    fn subset_of_known_hints_is_silent() {
        let mux = setup(&["invite"]);
        mux.note("invite", addr(1, &[1, 2]));
        let mut news = mux.subscribe();

        mux.note("invite", addr(1, &[2]));

        assert!(
            matches!(news.try_recv(), Err(TryRecvError::Empty)),
            "no new hints — no broadcast"
        );
    }

    #[test]
    fn every_subscriber_gets_the_news() {
        let mux = setup(&["invite"]);
        let (mut one, mut two) = (mux.subscribe(), mux.subscribe());

        mux.note("invite", addr(1, &[1]));

        assert_eq!(one.try_recv().ok(), Some(addr(1, &[1])));
        assert_eq!(two.try_recv().ok(), Some(addr(1, &[1])));
    }
}

#[path = "discovery_mux_run_tests.rs"]
mod running;

#[path = "discovery_mux_found_tests.rs"]
mod found;

/// Hint lifetime in tests (the project's `tokio` has no `time::pause`).
const TTL_MS: u64 = 200;

fn setup(order: &[&'static str]) -> DiscoveryMux {
    setup_with(order, TTL_MS)
}

fn setup_with(order: &[&'static str], ttl_ms: u64) -> DiscoveryMux {
    let timing = DiscoveryTiming {
        ttl: Duration::from_millis(ttl_ms),
        publish_timeout: Duration::from_secs(2),
    };
    DiscoveryMux::with_timing(order.to_vec(), timing)
}

fn hints(ns: &[u16]) -> Vec<AddrHint> {
    ns.iter().map(|n| hint(*n)).collect()
}

fn hints_of(mux: &DiscoveryMux, n: u8) -> Option<Vec<AddrHint>> {
    mux.lookup(&pid(n)).map(|found| found.hints)
}

/// A mux with `scripts` sources, already started.
fn started(order: &[&'static str], scripts: &[&Arc<Script>]) -> Arc<DiscoveryMux> {
    let mut mux = setup(order);
    for script in scripts {
        let source: Arc<dyn Discovery> = (*script).clone();
        mux.add(source);
    }
    let mux = Arc::new(mux);
    mux.start();
    mux
}

/// Waits until `lookup` of node `n` finds something (the deadline is up to the caller).
async fn wait_lookup(mux: &DiscoveryMux, n: u8) -> PeerAddr {
    let id: PeerId = pid(n);
    loop {
        if let Some(found) = mux.lookup(&id) {
            return found;
        }
        sleep(Duration::from_millis(10)).await;
    }
}

/// Waits until the condition becomes true, no longer than the step deadline.
async fn wait_until(what: &str, cond: impl Fn() -> bool) {
    within(what, async {
        while !cond() {
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
}
