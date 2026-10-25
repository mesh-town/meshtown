//! A node start that a stop request can interrupt.
//!
//! The start runs as its own task; the race polls the stop flag of the control object instead of reading
//! the command queue, so every command other than `Stop` is left for the cycle to handle after the start.
//! An interrupted start is handed to a background task that closes the node if it still comes up.

use std::future::Future;
use std::time::Duration;

use tokio::task::{AbortHandle, JoinHandle};
use tokio::time::{sleep, timeout};

use crate::node::node_ctl::NodeCtl;
use crate::node::output::{say, spawn};
use crate::transport::net::stream::BoxFuture;

/// How often the stop flag is checked while the start runs.
pub(super) const STOP_POLL: Duration = Duration::from_millis(50);

/// How long an interrupted start may keep running before it is dropped.
pub(super) const ABANDONED_START_LIMIT: Duration = Duration::from_secs(60);

/// Aborts the start task when the race is dropped from outside, unless the task was handed over.
struct AbortGuard(Option<AbortHandle>);

impl AbortGuard {
    fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for AbortGuard {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            handle.abort();
        }
    }
}

/// Runs `start` until it ends or a stop is requested. `Ok(None)` — the stop won: the start continues in the
/// background and `discard` closes whatever it produces. A finished start wins over a simultaneous stop.
pub(super) async fn start_unless_stopped<T, S, D>(
    ctl: &NodeCtl,
    start: S,
    discard: D,
) -> anyhow::Result<Option<T>>
where
    T: Send + 'static,
    S: Future<Output = anyhow::Result<T>> + Send + 'static,
    D: FnOnce(T) -> BoxFuture<'static, ()> + Send + 'static,
{
    let mut task = spawn(start);
    let mut guard = AbortGuard(Some(task.abort_handle()));
    loop {
        tokio::select! {
            biased;
            joined = &mut task => {
                guard.disarm();
                return finished(joined);
            }
            _ = sleep(STOP_POLL) => {
                if ctl.stop_requested() {
                    guard.disarm();
                    abandon(task, discard);
                    return Ok(None);
                }
            }
        }
    }
}

/// The result of a finished start task; a panic of the start goes on as a panic.
fn finished<T>(
    joined: Result<anyhow::Result<T>, tokio::task::JoinError>,
) -> anyhow::Result<Option<T>> {
    match joined {
        Ok(result) => result.map(Some),
        Err(e) if e.is_panic() => std::panic::resume_unwind(e.into_panic()),
        Err(_) => Err(anyhow::anyhow!("the node start was cancelled")),
    }
}

/// Waits in the background for an interrupted start: a node that comes up is handed to `discard`,
/// a failed start needs nothing, a start that does not end in time is aborted.
fn abandon<T, D>(mut task: JoinHandle<anyhow::Result<T>>, discard: D)
where
    T: Send + 'static,
    D: FnOnce(T) -> BoxFuture<'static, ()> + Send + 'static,
{
    spawn(async move {
        match timeout(ABANDONED_START_LIMIT, &mut task).await {
            Ok(Ok(Ok(node))) => discard(node).await,
            Ok(_) => {}
            Err(_) => {
                task.abort();
                say(
                    "node",
                    format!(
                        "an interrupted start did not end within {} s and was dropped",
                        ABANDONED_START_LIMIT.as_secs()
                    ),
                );
            }
        }
    });
}

#[cfg(test)]
#[path = "start_race_tests.rs"]
mod tests;
