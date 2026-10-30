//! Tests of the check schedule: interval offset and the "already being checked" lock (`schedule.rs`).

use super::*;
use crate::transport::net::{NodeSecret, PeerId};

#[test]
fn offset_is_stable_and_inside_interval() {
    let author = peer(7);
    let other = peer(9);
    let interval = Duration::from_secs(300);
    let a = offset_for(author, "example.com", interval);
    assert_eq!(a, offset_for(author, "example.com", interval));
    assert!(a < interval);
    assert!(offset_for(other, "example.com", interval) < interval);
    assert!(offset_for(author, "example.org", interval) < interval);
}

#[test]
fn first_check_is_within_a_minute_regardless_of_interval() {
    for seed in 1..=20 {
        for domain in ["example.com", "dom.example.net", "a.b.c.d.example.org"] {
            let d = first_check_delay(peer(seed), domain);
            assert!(
                d >= FIRST_DELAY && d < FIRST_DELAY + FIRST_SPREAD,
                "{seed} {domain}: {d:?}"
            );
        }
    }
}

#[test]
fn in_progress_blocks_second_try_begin() {
    let in_progress = InProgress::default();
    assert!(in_progress.try_begin("example.com"));
    assert!(!in_progress.try_begin("example.com"));
    in_progress.end("example.com");
    assert!(in_progress.try_begin("example.com"));
}

mod remember_verdict {
    use super::*;

    fn fresh() -> DomainState {
        DomainState {
            next_at: Instant::now(),
            last_verdict: None,
            same_verdict_streak: 0,
        }
    }

    #[test]
    fn same_grows() {
        let mut s = fresh();
        s.remember_verdict(CheckVerdict::Ok);
        s.remember_verdict(CheckVerdict::Ok);
        assert_eq!(s.same_verdict_streak, 1);
    }

    #[test]
    fn capped() {
        let mut s = fresh();
        for _ in 0..6 {
            s.remember_verdict(CheckVerdict::Ok);
        }
        assert_eq!(s.same_verdict_streak, 4);
    }

    #[test]
    fn other_resets() {
        let mut s = fresh();
        s.remember_verdict(CheckVerdict::Ok);
        s.remember_verdict(CheckVerdict::Down);
        assert_eq!(s.same_verdict_streak, 0);
        assert_eq!(s.last_verdict, Some(CheckVerdict::Down));
    }
}

mod backoff_interval {
    use super::*;

    #[test]
    fn doubles_per_streak_up_to_max() {
        let schedule = Schedule::default();
        assert_eq!(schedule.backoff_interval(0), Duration::from_secs(300));
        assert_eq!(schedule.backoff_interval(2), Duration::from_secs(1200));
        assert_eq!(schedule.backoff_interval(4), Duration::from_secs(3600));
    }
}

fn peer(seed: u8) -> PeerId {
    NodeSecret::from_bytes(&[seed; 32]).peer_id()
}
