//! Reaction to a network change: close sessions → `suspend` → `resume`.

use super::source::{NetEvent, NetEvents};
use crate::node::output::say;
use crate::transport::net::stream::BoxFuture;
use crate::transport::net::transport::{NET_CHANGED, NET_LOST};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::{sleep_until, Instant};

/// Hold-off from the first event: a burst of OS events merges into one reaction.
pub const SETTLE_WINDOW: Duration = Duration::from_millis(500);
/// Close code for a network change (close in meaning to `CODE_SILENT` = `0xFFFF_0002`).
pub const CODE_NET_CHANGED: u32 = 0xFFFF_0003;

/// Executor of the reaction (the real one is over the DHT, in tests a call recorder).
pub(crate) trait NetReact: Send + Sync {
    /// Close live sessions with a reason; how many were closed.
    fn close_sessions(&self, code: u32, reason: &[u8]) -> usize;
    fn suspend(&self) -> BoxFuture<'_, Result<(), String>>;
    fn resume(&self) -> BoxFuture<'_, Result<(), String>>;
}

/// Waits for an event; `None` — time to exit (stop, or the source ran dry).
async fn wait_event(
    events: &mut dyn NetEvents,
    stop: &mut watch::Receiver<bool>,
) -> Option<NetEvent> {
    if *stop.borrow() {
        return None;
    }
    tokio::select! {
        e = events.next() => e,
        r = stop.changed() => { let _ = r; None }
    }
}

/// Merges the events of the `SETTLE_WINDOW` window; the last one decides. The window is not extended.
async fn coalesce_event_burst(first: NetEvent, events: &mut dyn NetEvents) -> NetEvent {
    let deadline = Instant::now() + SETTLE_WINDOW;
    let mut last = first;
    loop {
        tokio::select! {
            _ = sleep_until(deadline) => return last,
            e = events.next() => match e {
                Some(e) => last = e,
                None => { sleep_until(deadline).await; return last }
            },
        }
    }
}

fn warn(what: &str, err: &str) {
    say("hyper", format!("{what}: {err}"));
}

async fn lost(react: &dyn NetReact, asleep: &mut bool) {
    if *asleep {
        return;
    }
    let n = react.close_sessions(CODE_NET_CHANGED, NET_LOST.as_bytes());
    match react.suspend().await {
        Ok(()) => *asleep = true,
        Err(e) => warn("suspend failed", &e),
    }
    say("hyper", format!("network lost: closed {n} sessions"));
}

async fn changed(react: &dyn NetReact, asleep: &mut bool) {
    let t0 = Instant::now();
    let n = react.close_sessions(CODE_NET_CHANGED, NET_CHANGED.as_bytes());
    if !*asleep {
        if let Err(e) = react.suspend().await {
            warn("suspend failed", &e);
        }
    }
    if let Err(e) = react.resume().await {
        warn("resume failed", &e);
    }
    *asleep = false;
    let ms = t0.elapsed().as_millis();
    say(
        "hyper",
        format!("network changed: closed {n} sessions, resuming in {ms} ms"),
    );
}

/// The reaction loop; exits on `stop`, on a dropped sender, or when the source ran dry.
pub(crate) async fn react_loop(
    mut events: Box<dyn NetEvents>,
    react: Arc<dyn NetReact>,
    mut stop: watch::Receiver<bool>,
) {
    let mut asleep = false;
    while let Some(first) = wait_event(events.as_mut(), &mut stop).await {
        match coalesce_event_burst(first, events.as_mut()).await {
            NetEvent::Lost => lost(react.as_ref(), &mut asleep).await,
            NetEvent::Changed => changed(react.as_ref(), &mut asleep).await,
        }
    }
}

#[cfg(test)]
#[path = "react_tests.rs"]
mod tests;
