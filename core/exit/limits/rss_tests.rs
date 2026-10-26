//! Tests of the cached memory check and of the ceiling formula.

use super::*;
use std::sync::atomic::AtomicUsize;

mod over {
    use super::*;

    #[test]
    fn below_the_ceiling_is_not_over() {
        // given: a process reading 999 KiB against a ceiling of 1000
        let gate = RssGate::new(1000, || 999);

        // then: not over
        assert_eq!(gate.over(Instant::now()), None);
    }

    #[test]
    fn at_the_ceiling_is_over() {
        // given: a process reading exactly the ceiling
        let gate = RssGate::new(1000, || 1000);

        // then: over, with the reading and the ceiling
        assert_eq!(gate.over(Instant::now()), Some((1000, 1000)));
    }

    #[test]
    fn zero_ceiling_never_reads() {
        // given: no ceiling and a reader that counts its calls
        static READS: AtomicUsize = AtomicUsize::new(0);
        fn read() -> u64 {
            READS.fetch_add(1, Ordering::SeqCst);
            u64::MAX
        }
        let gate = RssGate::new(0, read);

        // when: asked
        let answer = gate.over(Instant::now());

        // then: not over and the memory was not read
        assert_eq!(answer, None);
        assert_eq!(READS.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn unknown_rss_is_not_over() {
        // given: a reader with no data (0)
        let gate = RssGate::new(1000, || 0);

        // then: not over
        assert_eq!(gate.over(Instant::now()), None);
    }

    #[test]
    fn reads_at_most_once_per_second() {
        // given: a reader that counts its calls
        static READS: AtomicUsize = AtomicUsize::new(0);
        fn read() -> u64 {
            READS.fetch_add(1, Ordering::SeqCst);
            10
        }
        let gate = RssGate::new(1000, read);
        let start = Instant::now();

        // when: asked twice within a second, then after a second
        gate.over(start);
        gate.over(start + Duration::from_millis(900));
        let after_cache = READS.load(Ordering::SeqCst);
        gate.over(start + Duration::from_secs(1));

        // then: the second ask came from the cache, the third read anew
        assert_eq!(after_cache, 1, "the second ask is cached");
        assert_eq!(
            READS.load(Ordering::SeqCst),
            2,
            "a new reading after a second"
        );
    }
}

mod ceiling {
    use super::*;

    #[test]
    fn zero_warn_means_no_ceiling() {
        assert_eq!(rss_ceiling_kib(0), 0);
    }

    #[test]
    fn leaves_one_mib_below_the_warn_level() {
        assert_eq!(rss_ceiling_kib(24), 23 * 1024);
    }

    #[test]
    fn never_drops_below_one_mib() {
        assert_eq!(rss_ceiling_kib(1), 1024);
    }
}
