//! `first_ok`: the first success of the key dial or of any direct attempt wins; losers are dropped.

use super::*;
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::stream::BoxFuture;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

mod without_direct_attempts {
    use super::*;

    #[tokio::test]
    async fn returns_the_key_value() {
        let res = first_ok(ok_after(1, 0), vec![]).await;
        assert!(matches!(res, Ok(1)));
    }

    #[tokio::test]
    async fn returns_the_key_error() {
        let res = first_ok(err_after(NetError::NoRoute, 0), vec![]).await;
        assert!(matches!(res, Err(NetError::NoRoute)));
    }
}

mod key_fails_direct_succeeds {
    use super::*;

    #[tokio::test]
    async fn returns_the_direct_value() {
        // given
        let key = err_after(NetError::NoRoute, 0);
        let direct = vec![ok_after(2, 50)];
        // when
        let res = first_ok(key, direct).await;
        // then
        assert!(matches!(res, Ok(2)));
    }
}

mod direct_fails_key_succeeds {
    use super::*;

    #[tokio::test]
    async fn returns_the_key_value_without_waiting_for_pending_directs() {
        // given
        let dropped = Arc::new(AtomicBool::new(false));
        let direct = vec![pending_with_guard(dropped.clone())];
        let started = Instant::now();
        // when
        let res = first_ok(ok_after(1, 0), direct).await;
        // then
        assert!(matches!(res, Ok(1)));
        assert!(
            started.elapsed() < Duration::from_millis(100),
            "the key result waited for a pending direct attempt"
        );
        assert!(
            dropped.load(Ordering::SeqCst),
            "the losing attempt must be dropped with the race"
        );
    }
}

mod direct_succeeds_key_pending {
    use super::*;

    #[tokio::test]
    async fn drops_the_pending_key_attempt() {
        // given
        let dropped = Arc::new(AtomicBool::new(false));
        let key = pending_with_guard(dropped.clone());
        // when
        let res = first_ok(key, vec![ok_after(3, 20)]).await;
        // then
        assert!(matches!(res, Ok(3)));
        assert!(
            dropped.load(Ordering::SeqCst),
            "the key attempt must be dropped"
        );
    }
}

mod all_fail {
    use super::*;

    #[tokio::test]
    async fn returns_the_key_error() {
        // given: the direct attempt fails first, the key a little later
        let key = err_after(NetError::Timeout, 30);
        let direct = vec![err_after(NetError::NoRoute, 0)];
        // when
        let res = first_ok(key, direct).await;
        // then
        assert!(matches!(res, Err(NetError::Timeout)));
    }
}

mod direct_fails_first {
    use super::*;

    #[tokio::test]
    async fn waits_for_the_key() {
        // given
        let key = ok_after(1, 50);
        let direct = vec![err_after(NetError::NoRoute, 0)];
        // when
        let res = first_ok(key, direct).await;
        // then
        assert!(matches!(res, Ok(1)));
    }
}

mod several_direct_attempts {
    use super::*;

    #[tokio::test]
    async fn the_one_that_succeeds_wins_over_failed_ones() {
        // given
        let key = err_after(NetError::NoRoute, 0);
        let direct = vec![err_after(NetError::Timeout, 0), ok_after(4, 40)];
        // when
        let res = first_ok(key, direct).await;
        // then
        assert!(matches!(res, Ok(4)));
    }
}

fn ok_after(value: u32, ms: u64) -> BoxFuture<'static, NetResult<u32>> {
    Box::pin(async move {
        sleep(Duration::from_millis(ms)).await;
        Ok(value)
    })
}

fn err_after(err: NetError, ms: u64) -> BoxFuture<'static, NetResult<u32>> {
    Box::pin(async move {
        sleep(Duration::from_millis(ms)).await;
        Err(err)
    })
}

/// Never finishes; sets `dropped` when the future is dropped.
fn pending_with_guard(dropped: Arc<AtomicBool>) -> BoxFuture<'static, NetResult<u32>> {
    let guard = DropFlag(dropped);
    Box::pin(async move {
        let _guard = guard;
        std::future::pending().await
    })
}

struct DropFlag(Arc<AtomicBool>);

impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
