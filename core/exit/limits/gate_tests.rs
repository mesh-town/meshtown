//! Tests of the live-stream counter: the cap, the slot guard, the memory refusal and the refusal log pacing.

use super::*;
use std::time::Duration;

mod admit {
    use super::*;

    #[test]
    fn admits_up_to_the_limit_and_refuses_the_next() {
        // given: a cap of 3
        let limits = ExitLimits::fixed(3);

        // when: four streams ask for a place
        let answers: Vec<_> = (0..4).map(|_| limits.try_admit()).collect();

        // then: three are admitted, the fourth is refused with the counts, and three stay live
        assert!(
            answers[..3].iter().all(|a| a.is_ok()),
            "first three admitted"
        );
        assert!(
            matches!(answers[3], Err(Busy::Streams { live: 3, limit: 3 })),
            "the fourth is refused by the cap"
        );
        assert_eq!(limits.live(), 3);
    }

    #[test]
    fn zero_setting_uses_the_machine_default() {
        // given: a limiter configured with the setting 0 and the automatic cap 2
        let limits = ExitLimits::fixed(100);
        limits.configure(0, 2, 0);

        // when: three streams ask for a place
        let answers: Vec<_> = (0..3).map(|_| limits.try_admit()).collect();

        // then: two are admitted, the third is refused
        assert!(answers[0].is_ok() && answers[1].is_ok(), "two admitted");
        assert!(answers[2].is_err(), "the third is refused");
        assert_eq!(limits.max_streams(), 2);
    }

    #[test]
    fn configure_sets_the_rss_ceiling() {
        // given: a limiter without a memory ceiling whose process reads 2000 KiB
        let limits = ExitLimits::with_rss(10, 0, || 2000);

        // when: it is configured with the ceiling 1000 KiB and the automatic cap 5
        limits.configure(0, 5, 1000);
        let answer = limits.try_admit();

        // then: the memory refuses the stream and the cap follows the automatic value
        assert!(
            matches!(answer, Err(Busy::Memory { .. })),
            "refused by memory"
        );
        assert_eq!(limits.max_streams(), 5);
    }
}

mod live {
    use super::*;

    #[test]
    fn raising_the_limit_admits_again() {
        // given: a cap of 1 and its only place taken
        let limits = ExitLimits::fixed(1);
        let _held = limits.try_admit().expect("the first place");
        assert!(limits.try_admit().is_err(), "full");

        // when: the cap is raised to 2
        limits.set_max_streams(2);

        // then: a new stream is admitted
        assert!(limits.try_admit().is_ok(), "admitted after raising");
    }

    #[test]
    fn zero_restores_the_auto_limit() {
        // given: a limiter configured with the setting 5 and the automatic cap 2
        let limits = ExitLimits::fixed(1);
        limits.configure(5, 2, 0);
        assert_eq!(limits.max_streams(), 5);

        // when: the setting becomes 0
        limits.set_max_streams(0);

        // then: the automatic cap is back
        assert_eq!(limits.max_streams(), 2);
    }
}

mod slot {
    use super::*;

    #[test]
    fn dropping_a_slot_frees_it() {
        // given: a cap of 1 and its place taken
        let limits = ExitLimits::fixed(1);
        let held = limits.try_admit().expect("the place");
        assert_eq!(limits.live(), 1);

        // when: the slot is dropped
        drop(held);

        // then: no live streams and a new one fits
        assert_eq!(limits.live(), 0);
        assert!(limits.try_admit().is_ok(), "the place is free again");
    }

    #[tokio::test]
    async fn aborted_task_frees_its_slot() {
        // given: a task that holds a slot and never ends
        let limits = ExitLimits::fixed(1);
        let held = limits.try_admit().expect("the place");
        let task = crate::transport::net::task_label::spawn(async move {
            let _slot = held;
            std::future::pending::<()>().await;
        });
        assert_eq!(limits.live(), 1);

        // when: the task is aborted and awaited
        task.abort();
        let joined = task.await;

        // then: it was cancelled and the place is free
        assert!(joined.err().is_some_and(|e| e.is_cancelled()), "cancelled");
        assert_eq!(limits.live(), 0);
    }
}

mod rss {
    use super::*;

    #[test]
    fn over_the_ceiling_refuses_and_keeps_the_count() {
        // given: a process reading 2000 KiB against a ceiling of 1000 KiB
        let limits = ExitLimits::with_rss(10, 1000, || 2000);

        // when: a stream asks for a place
        let answer = limits.try_admit();

        // then: refused by memory with both numbers, and no place stays taken
        let Err(Busy::Memory {
            rss_kib,
            ceiling_kib,
        }) = answer
        else {
            panic!("expected a memory refusal");
        };
        assert_eq!((rss_kib, ceiling_kib), (2000, 1000));
        assert_eq!(limits.live(), 0);
    }
}

mod log {
    use super::*;

    #[test]
    fn busy_line_at_most_every_ten_seconds() {
        // given: the first refusal
        let limits = ExitLimits::fixed(1);
        let start = Instant::now();
        let first = limits.busy_line_due(start);

        // when: a second one comes in the same second, a third ten seconds later
        let second = limits.busy_line_due(start + Duration::from_secs(1));
        let third = limits.busy_line_due(start + Duration::from_secs(10));

        // then: the first gets a line, the second waits, the third reports the one it counted
        assert_eq!(first, Some(0));
        assert_eq!(second, None);
        assert_eq!(third, Some(1));
    }
}

mod busy_text {
    use super::*;

    #[test]
    fn streams_line_names_live_count_and_limit() {
        let text = Busy::Streams {
            live: 20,
            limit: 20,
        }
        .line();
        assert_eq!(text, "busy: 20 streams open, limit 20");
    }

    #[test]
    fn memory_line_names_sizes_in_mib() {
        let text = Busy::Memory {
            rss_kib: 30 * 1024,
            ceiling_kib: 23 * 1024,
        }
        .line();
        assert_eq!(text, "busy: memory 30 MiB, ceiling 23 MiB");
    }
}
