//! Tests for `limiter.rs`: failure window, blocks, doubling, table limit.

use super::*;
use std::net::Ipv4Addr;

fn ip(last: u8) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(192, 168, 1, last))
}

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

fn fail_n(l: &mut Limiter, who: IpAddr, n: u32, now: Instant) {
    for _ in 0..n {
        l.record_fail(who, now);
    }
}

mod fresh {
    use super::*;

    #[test]
    fn four_fails_still_allowed() {
        let t0 = Instant::now();
        let mut l = Limiter::new();
        fail_n(&mut l, ip(10), 4, t0);
        assert_eq!(l.check_not_blocked(ip(10), t0), Ok(()));
    }

    #[test]
    fn fifth_fail_blocks_60s() {
        let t0 = Instant::now();
        let mut l = Limiter::new();
        fail_n(&mut l, ip(10), 5, t0);
        assert_eq!(l.check_not_blocked(ip(10), t0), Err(secs(60)));
        assert_eq!(l.check_not_blocked(ip(10), t0 + secs(60)), Ok(()));
    }

    #[test]
    fn fails_spread_over_window_do_not_block() {
        let t0 = Instant::now();
        let mut l = Limiter::new();
        for i in 0..5u64 {
            l.record_fail(ip(10), t0 + secs(i * 240));
        }
        assert_eq!(l.check_not_blocked(ip(10), t0 + secs(960)), Ok(()));
    }

    #[test]
    fn other_ip_unaffected() {
        let t0 = Instant::now();
        let mut l = Limiter::new();
        fail_n(&mut l, ip(10), 5, t0);
        assert_eq!(l.check_not_blocked(ip(11), t0), Ok(()));
    }
}

mod after_block {
    use super::*;

    fn blocked(t0: Instant) -> Limiter {
        let mut l = Limiter::new();
        fail_n(&mut l, ip(10), 5, t0);
        l
    }

    #[test]
    fn next_fail_blocks_120s() {
        let t0 = Instant::now();
        let mut l = blocked(t0);
        l.record_fail(ip(10), t0 + secs(61));
        assert_eq!(l.check_not_blocked(ip(10), t0 + secs(61)), Err(secs(120)));
    }

    #[test]
    fn doubling_caps_at_15_min() {
        let t0 = Instant::now();
        let mut l = blocked(t0);
        let mut now = t0;
        let mut waits = Vec::new();
        for _ in 0..4 {
            now += l.check_not_blocked(ip(10), now).unwrap_err();
            l.record_fail(ip(10), now);
            waits.push(l.check_not_blocked(ip(10), now).unwrap_err());
        }
        assert_eq!(waits, vec![secs(120), secs(240), secs(480), secs(900)]);
    }

    #[test]
    fn quiet_15_min_starts_fresh() {
        let t0 = Instant::now();
        let mut l = blocked(t0);
        let later = t0 + secs(900 + 60);
        l.record_fail(ip(10), later);
        assert_eq!(l.check_not_blocked(ip(10), later), Ok(()));
        fail_n(&mut l, ip(10), 3, later);
        assert_eq!(l.check_not_blocked(ip(10), later), Ok(()));
    }

    #[test]
    fn succeed_resets_all() {
        let t0 = Instant::now();
        let mut l = blocked(t0);
        l.clear_fails(ip(10));
        assert_eq!(l.check_not_blocked(ip(10), t0), Ok(()));
        assert_eq!(l.len(), 0);
    }
}

mod table {
    use super::*;

    fn addr(n: u32) -> IpAddr {
        IpAddr::V4(Ipv4Addr::from(0x0a00_0000 + n))
    }

    #[test]
    fn never_exceeds_1024() {
        let t0 = Instant::now();
        let mut l = Limiter::new();
        for n in 0..1500u32 {
            l.record_fail(addr(n), t0 + Duration::from_millis(u64::from(n)));
        }
        assert_eq!(l.len(), 1024);
    }

    #[test]
    fn stale_entries_go_first() {
        let t0 = Instant::now();
        let mut l = Limiter::new();
        for n in 0..1024u32 {
            l.record_fail(addr(n), t0);
        }
        let later = t0 + secs(2000);
        l.record_fail(ip(10), later);
        assert_eq!(l.len(), 1);
    }

    #[test]
    fn oldest_last_fail_goes_when_all_fresh() {
        let t0 = Instant::now();
        let mut l = Limiter::new();
        for n in 0..1024u32 {
            l.record_fail(addr(n), t0 + Duration::from_millis(u64::from(n)));
        }
        let now = t0 + secs(5);
        l.record_fail(ip(10), now);
        assert_eq!(l.len(), 1024);
        fail_n(&mut l, addr(1023), 4, now);
        assert_eq!(l.check_not_blocked(addr(1023), now), Err(secs(60)));
    }
}
