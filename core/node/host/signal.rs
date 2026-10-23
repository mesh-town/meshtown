//! The OS stop signal: one task per process, started by the host.

use crate::node::output::say;

#[cfg(unix)]
async fn wait_for_stop_signal() {
    use tokio::signal::unix::{signal, SignalKind};
    let mut term = match signal(SignalKind::terminate()) {
        Ok(s) => s,
        Err(_) => {
            let _ = tokio::signal::ctrl_c().await;
            return;
        }
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        _ = term.recv() => {},
    }
}

#[cfg(not(unix))]
async fn wait_for_stop_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

/// Stop signal task: waits for Ctrl-C/SIGTERM, then calls `stop` (the host's `stop_all_on_signal`, which stops
/// every slot without the page-request line). One for the whole process —
/// otherwise either two signal tasks (a double line and two `Stop`s),
/// or a gap between waiting and the node's work in which Ctrl-C would be lost.
pub(crate) fn spawn_stop_signal(
    stop: impl FnOnce() + Send + 'static,
) -> tokio::task::JoinHandle<()> {
    crate::node::output::spawn(async move {
        wait_for_stop_signal().await;
        say("node", "received stop signal");
        stop();
    })
}
