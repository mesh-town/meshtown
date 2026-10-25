//! The wait for a replica flush on shutdown: done, failed, or stuck past the limit.
//!
//! Time is paused, so a stuck flush costs no real waiting.

use std::future::{pending, ready};

use tokio::time::Instant;

use super::*;
use crate::transport::net::{NetError, NetResult};

#[tokio::test(start_paused = true)]
async fn fast_flush_has_no_trouble() {
    let trouble = flush_trouble(ready(Ok(())), REPLICA_FLUSH_LIMIT).await;

    assert_eq!(trouble, None);
}

#[tokio::test(start_paused = true)]
async fn stuck_flush_gives_up_after_the_limit() {
    // given: a flush that never ends
    let began = Instant::now();

    // when
    let trouble = flush_trouble(pending::<NetResult<()>>(), REPLICA_FLUSH_LIMIT).await;

    // then: the problem names the timeout, and the wait lasted the limit and no longer
    let text = trouble.expect("a stuck flush is a problem");
    assert!(text.contains("did not finish"), "text: {text}");
    let waited = began.elapsed();
    assert!(
        waited >= REPLICA_FLUSH_LIMIT && waited < REPLICA_FLUSH_LIMIT + Duration::from_millis(5),
        "waited {waited:?}, limit {REPLICA_FLUSH_LIMIT:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn failed_flush_reports_the_error() {
    let failed = ready(Err(NetError::Other(anyhow::anyhow!("disk full"))));

    let trouble = flush_trouble(failed, REPLICA_FLUSH_LIMIT).await;

    let text = trouble.expect("a failed flush is a problem");
    assert!(text.contains("failed"), "text: {text}");
    assert!(text.contains("disk full"), "text: {text}");
}
