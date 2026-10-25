//! Tests for the account-record discovery round, split out of `source_tests.rs`.

use super::*;
use crate::transport::net_mainline::record::RosterEntry;

mod successful_round {
    use super::*;

    #[test]
    fn returns_the_other_keys_without_mine_in_record_order() {
        let mut f = setup();
        let got = f.when_account_ok(vec![SEED_A, SEED_C, SEED_B]);
        assert_eq!(got, Some(vec![pid(SEED_A), pid(SEED_B)]));
    }

    #[test]
    fn resets_the_failure_streak() {
        let mut f = setup();
        f.failures = 2;
        f.when_account_ok(vec![SEED_A, SEED_C]);
        assert_eq!(f.failures, 0, "a successful round clears the streak");
    }

    #[test]
    fn tally_counts_one_round_and_no_error() {
        let mut f = setup();
        f.when_account_ok(vec![SEED_A, SEED_C]);
        assert_eq!(f.tally.counts(), (1, 0));
    }

    #[test]
    fn reports_read_members_and_published() {
        let mut f = setup();
        f.when_account_ok(vec![SEED_A, SEED_B, SEED_C]);
        assert_eq!(
            f.outcomes(),
            vec![DhtRoundOutcome {
                read: true,
                members: 3,
                published: true
            }]
        );
    }
}

mod failed_round {
    use super::*;

    #[test]
    fn returns_nothing() {
        let mut f = setup();
        assert_eq!(f.when_account_timeout(), None);
    }

    #[test]
    fn counts_one_more_failure() {
        let mut f = setup();
        f.when_account_timeout();
        assert_eq!(f.failures, 1);
    }

    #[test]
    fn failures_accumulate_across_rounds() {
        let mut f = setup();
        f.failures = 1;
        f.when_account_timeout();
        assert_eq!(f.failures, 2);
    }

    #[test]
    fn tally_counts_one_round_and_one_error() {
        let mut f = setup();
        f.when_account_timeout();
        assert_eq!(f.tally.counts(), (1, 1));
    }

    #[test]
    fn reports_not_read_with_no_members() {
        let mut f = setup();
        f.when_account_timeout();
        assert_eq!(
            f.outcomes(),
            vec![DhtRoundOutcome {
                read: false,
                members: 0,
                published: false
            }]
        );
    }
}

struct Fixture {
    source: HyperDhtDiscovery,
    tally: Tally,
    failures: u32,
    seen: Arc<std::sync::Mutex<Vec<DhtRoundOutcome>>>,
}

impl Fixture {
    fn when_account_ok(&mut self, ids: Vec<u8>) -> Option<Vec<PeerId>> {
        let entries = ids.into_iter().map(entry).collect();
        let roster = Roster {
            version: crate::transport::net_mainline::record::RECORD_VERSION_KEYS,
            entries,
        };
        self.source
            .account(Ok(roster), &mut self.tally, &mut self.failures)
    }

    fn when_account_timeout(&mut self) -> Option<Vec<PeerId>> {
        let res = Err(NetError::Timeout);
        self.source
            .account(res, &mut self.tally, &mut self.failures)
    }

    fn outcomes(&self) -> Vec<DhtRoundOutcome> {
        self.seen.lock().unwrap().clone()
    }
}

/// A publishing source whose own key is `SEED_C`, with a fresh tally and a zero failure streak.
fn setup() -> Fixture {
    let (f, seen) = recorder();
    let source = HyperDhtDiscovery::new(Arc::new(MemRecords::new()), keys(), Some(pid(SEED_C)))
        .with_pace(test_pace())
        .with_round_outcome(f);
    let tally = Tally::new(Instant::now(), Duration::from_secs(600));
    Fixture {
        source,
        tally,
        failures: 0,
        seen,
    }
}

fn entry(seed: u8) -> RosterEntry {
    RosterEntry {
        id: *pid(seed).as_bytes(),
        relay: None,
        addrs: Vec::new(),
        seen_unix: 1,
    }
}
