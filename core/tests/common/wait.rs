//! Polling helper shared by the loopback scenarios, included as
//! `#[path = "common/wait.rs"] mod wait;`.

use std::time::Duration;

use tokio::time::{sleep, Instant};

/// Polls the condition every 50 ms until it becomes true or `limit` expires.
pub(crate) async fn wait_until(limit: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + limit;
    loop {
        if cond() {
            return true;
        }
        if Instant::now() >= deadline {
            return cond();
        }
        sleep(Duration::from_millis(50)).await;
    }
}
