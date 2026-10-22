//! The watcher of the membership list: recomputes the verdicts when the shared or the own replica
//! changes and on a timer (witnesses for new neighbours).
//!
//! What it does: one background task that calls `Access::refresh`.
//! What it doesn't do: no tally and no records of its own (`runtime`).

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tokio::task::AbortHandle;

use crate::net::ReplicaEvent;
use crate::output::say;

use super::runtime::Access;

const DEBOUNCE: Duration = Duration::from_millis(300);
const TICK: Duration = Duration::from_secs(60);

/// Starts the watcher; the first recompute publishes the view and lets the handshakes continue.
pub fn spawn_access(access: Arc<Access>) -> [AbortHandle; 1] {
    [tokio::spawn(run_watch(access)).abort_handle()]
}

/// What a received event means for the loop.
enum Wake {
    Recompute,
    Skip,
    Stop,
}

fn wake_of(event: Result<ReplicaEvent, RecvError>) -> Wake {
    match event {
        Ok(ReplicaEvent::LocalChange) | Ok(ReplicaEvent::RemoteChange) => Wake::Recompute,
        Err(RecvError::Lagged(_)) => Wake::Recompute,
        Err(RecvError::Closed) => Wake::Stop,
        Ok(_) => Wake::Skip,
    }
}

/// Drops the events queued while the debounce slept.
fn drain(rx: &mut Receiver<ReplicaEvent>) {
    while rx.try_recv().is_ok() {}
}

async fn refresh_logged(access: &Access) {
    if let Err(e) = access.refresh().await {
        say("access", format!("acl recompute failed: {e}"));
    }
}

/// Waits for the next reason to recompute; `false` — a replica is gone, stop.
async fn next_wake(shared: &mut Receiver<ReplicaEvent>, own: &mut Receiver<ReplicaEvent>, tick: &mut tokio::time::Interval) -> bool {
    loop {
        let wake = tokio::select! {
            e = shared.recv() => wake_of(e),
            e = own.recv() => wake_of(e),
            _ = tick.tick() => Wake::Recompute,
        };
        match wake {
            Wake::Recompute => return true,
            Wake::Stop => return false,
            Wake::Skip => {}
        }
    }
}

async fn run_watch(access: Arc<Access>) {
    watch_every(access, TICK).await;
}

/// The loop of the watcher with the period of the timer as an argument; the first tick fires at once.
async fn watch_every(access: Arc<Access>, tick: Duration) {
    let mut shared = access.shared.subscribe();
    let mut own = access.own.subscribe();
    let mut tick = tokio::time::interval(tick);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    while next_wake(&mut shared, &mut own, &mut tick).await {
        tokio::time::sleep(DEBOUNCE).await;
        drain(&mut shared);
        drain(&mut own);
        refresh_logged(&access).await;
    }
}

#[cfg(test)]
#[path = "watch_tests.rs"]
mod tests;
