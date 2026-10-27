//! `direct_then_relay`: the direct phase first, the relay phase when it fails or runs out of time, and the
//! budgets of both phases. The clock is paused, so every limit is checked to the second.

use super::*;
use crate::transport::net_hyper::transport::{DIRECT_LIMIT, RELAY_LIMIT};
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Instant};

const SECOND: Duration = Duration::from_secs(1);

mod direct_succeeds {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn returns_its_value_and_never_polls_the_relay() {
        // given
        let direct = Some(ok_after(7, 1));
        let relay = Some(panics());
        // when
        let res = direct_then_relay(direct, DIRECT_LIMIT, relay, RELAY_LIMIT).await;
        // then
        assert!(matches!(res, Ok((7, Phase::Direct))));
    }

    #[tokio::test(start_paused = true)]
    async fn takes_the_time_of_the_direct_phase_only() {
        // given
        let started = Instant::now();
        // when
        let _ = direct_then_relay(
            Some(ok_after(7, 3)),
            DIRECT_LIMIT,
            Some(panics()),
            RELAY_LIMIT,
        )
        .await;
        // then
        assert_eq!(started.elapsed(), 3 * SECOND);
    }
}

mod direct_times_out {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn direct_timeout_starts_relay() {
        // given
        let relay_start = Arc::new(Mutex::new(None));
        let started = Instant::now();
        let relay = Some(ok_noting_start(9, relay_start.clone()));
        // when
        let res = direct_then_relay(Some(hangs()), DIRECT_LIMIT, relay, RELAY_LIMIT).await;
        // then
        assert!(matches!(res, Ok((9, Phase::Relay))));
        let at = relay_start.lock().unwrap().expect("the relay phase ran") - started;
        assert_eq!(
            at, DIRECT_LIMIT,
            "the relay starts when the direct limit is over"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn both_phases_hanging_end_with_a_timeout_at_the_sum_of_the_limits() {
        // given
        let started = Instant::now();
        // when
        let res =
            direct_then_relay::<u32>(Some(hangs()), DIRECT_LIMIT, Some(hangs()), RELAY_LIMIT).await;
        // then
        assert!(matches!(res, Err(NetError::Timeout)));
        assert_eq!(started.elapsed(), DIRECT_LIMIT + RELAY_LIMIT);
    }
}

mod direct_fails_at_once {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn direct_error_starts_relay_at_once() {
        // given
        let relay_start = Arc::new(Mutex::new(None));
        let started = Instant::now();
        let relay = Some(ok_noting_start(9, relay_start.clone()));
        // when
        let res = direct_then_relay(
            Some(fails(NetError::NoRoute)),
            DIRECT_LIMIT,
            relay,
            RELAY_LIMIT,
        )
        .await;
        // then
        assert!(matches!(res, Ok((9, Phase::Relay))));
        let at = relay_start.lock().unwrap().expect("the relay phase ran") - started;
        assert_eq!(at, Duration::ZERO, "a refusal does not wait for the limit");
    }
}

mod direct_is_closed {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn closed_never_tries_relay() {
        // given
        let direct = Some(fails(NetError::Closed));
        // when
        let res = direct_then_relay(direct, DIRECT_LIMIT, Some(panics()), RELAY_LIMIT).await;
        // then
        assert!(matches!(res, Err(NetError::Closed)));
    }
}

mod relay_hangs {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn slow_relay_is_cut_and_direct_error_returned() {
        // given
        let started = Instant::now();
        let direct = Some(fails(NetError::NoRoute));
        // when
        let res = direct_then_relay::<u32>(direct, DIRECT_LIMIT, Some(hangs()), RELAY_LIMIT).await;
        // then
        assert!(matches!(res, Err(NetError::NoRoute)), "got {res:?}");
        assert_eq!(started.elapsed(), RELAY_LIMIT);
    }
}

mod relay_fails {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn the_error_of_the_direct_phase_wins() {
        // given
        let direct = Some(fails(NetError::NoRoute));
        let relay = Some(fails(NetError::Closed));
        // when
        let res = direct_then_relay::<u32>(direct, DIRECT_LIMIT, relay, RELAY_LIMIT).await;
        // then
        assert!(matches!(res, Err(NetError::NoRoute)), "got {res:?}");
    }
}

mod without_relay {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn without_relay_direct_keeps_its_limit() {
        // given: a dial without candidates runs under the old limit
        let started = Instant::now();
        let limit = 10 * SECOND;
        // when
        let res = direct_then_relay::<u32>(Some(hangs()), limit, None, RELAY_LIMIT).await;
        // then
        assert!(matches!(res, Err(NetError::Timeout)), "got {res:?}");
        assert_eq!(started.elapsed(), limit);
    }

    #[tokio::test(start_paused = true)]
    async fn direct_error_is_returned_as_it_is() {
        // given
        let direct = Some(fails(NetError::NoRoute));
        // when
        let res = direct_then_relay::<u32>(direct, DIRECT_LIMIT, None, RELAY_LIMIT).await;
        // then
        assert!(matches!(res, Err(NetError::NoRoute)), "got {res:?}");
    }
}

mod skipped_direct {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn skipped_direct_returns_relay_error() {
        // given
        let relay = Some(fails(NetError::NoRoute));
        // when
        let res = direct_then_relay::<u32>(None, DIRECT_LIMIT, relay, RELAY_LIMIT).await;
        // then
        assert!(matches!(res, Err(NetError::NoRoute)), "got {res:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn returns_the_relay_value_marked_as_relay() {
        // when
        let res = direct_then_relay(None, DIRECT_LIMIT, Some(ok_after(5, 2)), RELAY_LIMIT).await;
        // then
        assert!(matches!(res, Ok((5, Phase::Relay))));
    }

    #[tokio::test(start_paused = true)]
    async fn relay_has_its_own_limit() {
        // given
        let started = Instant::now();
        // when
        let res = direct_then_relay::<u32>(None, DIRECT_LIMIT, Some(hangs()), RELAY_LIMIT).await;
        // then
        assert!(matches!(res, Err(NetError::Timeout)), "got {res:?}");
        assert_eq!(started.elapsed(), RELAY_LIMIT);
    }
}

mod no_phases {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn is_no_route() {
        // when
        let res = direct_then_relay::<u32>(None, DIRECT_LIMIT, None, RELAY_LIMIT).await;
        // then
        assert!(matches!(res, Err(NetError::NoRoute)), "got {res:?}");
    }
}

// --- setup and helpers ---

type Attempt = BoxFuture<'static, NetResult<u32>>;

/// An attempt that succeeds with `value` after `secs` seconds.
fn ok_after(value: u32, secs: u64) -> Attempt {
    Box::pin(async move {
        sleep(Duration::from_secs(secs)).await;
        Ok(value)
    })
}

/// An attempt that fails with `err` at once.
fn fails(err: NetError) -> Attempt {
    Box::pin(async move { Err(err) })
}

/// An attempt that never finishes.
fn hangs() -> Attempt {
    Box::pin(std::future::pending())
}

/// An attempt that must never be polled.
fn panics() -> Attempt {
    async fn never_polled() -> NetResult<u32> {
        panic!("this phase must not run")
    }
    Box::pin(never_polled())
}

/// An attempt that notes when it was first polled and succeeds with `value`.
fn ok_noting_start(value: u32, at: Arc<Mutex<Option<Instant>>>) -> Attempt {
    Box::pin(async move {
        *at.lock().unwrap() = Some(Instant::now());
        Ok(value)
    })
}
