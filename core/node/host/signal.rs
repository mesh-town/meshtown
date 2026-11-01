//! The OS stop signal. Two tasks can listen for it in one `run` process: an early one, installed before the first
//! log line, that ends the process cleanly while the host is not yet running, and the host's own
//! (`spawn_stop_signal`), which stops the nodes gracefully. The host's task takes over once it has registered.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::node::output::say;

/// Set when the host has registered its own signal streams; the early task then leaves the signal to it.
static HOST_TOOK_OVER: AtomicBool = AtomicBool::new(false);

/// The registered stop-signal streams (SIGINT and SIGTERM on Unix, Ctrl-C elsewhere).
struct StopSignals {
    #[cfg(unix)]
    streams: Option<(tokio::signal::unix::Signal, tokio::signal::unix::Signal)>,
}

impl StopSignals {
    /// Registers the streams in the calling thread, so a signal that arrives later is kept for `wait`.
    /// Needs a running tokio runtime. A failed registration falls back to `ctrl_c` in `wait`.
    #[cfg(unix)]
    fn register() -> StopSignals {
        use tokio::signal::unix::{signal, SignalKind};
        let streams = signal(SignalKind::terminate())
            .and_then(|term| Ok((term, signal(SignalKind::interrupt())?)))
            .ok();
        StopSignals { streams }
    }

    #[cfg(not(unix))]
    fn register() -> StopSignals {
        StopSignals {}
    }

    #[cfg(unix)]
    async fn wait(self) {
        let Some((mut term, mut int)) = self.streams else {
            let _ = tokio::signal::ctrl_c().await;
            return;
        };
        tokio::select! {
            _ = term.recv() => {},
            _ = int.recv() => {},
        }
    }

    #[cfg(not(unix))]
    async fn wait(self) {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// Makes SIGINT/SIGTERM end the process with code 0 from now on, until the host takes the signal over: the first
/// log line of `run` is out and the start can take a while (probing the page port, binding the transports), and
/// the default action would kill the process without a trace. Registers in the calling thread, before any `.await`.
pub(crate) fn install_early_stop_signal() {
    let signals = StopSignals::register();
    crate::node::output::spawn(async move {
        signals.wait().await;
        if HOST_TOOK_OVER.load(Ordering::SeqCst) {
            return;
        }
        say("node", "received stop signal");
        std::process::exit(0);
    });
}

/// Stop signal task: waits for Ctrl-C/SIGTERM, then calls `stop` (the host's `stop_all_on_signal`, which stops
/// every slot without the page-request line). One for the whole process —
/// otherwise either two signal tasks (a double line and two `Stop`s),
/// or a gap between waiting and the node's work in which Ctrl-C would be lost.
/// The streams are registered before the early task is told to stand down, so no signal falls in between.
pub(crate) fn spawn_stop_signal(
    stop: impl FnOnce() + Send + 'static,
) -> tokio::task::JoinHandle<()> {
    let signals = StopSignals::register();
    HOST_TOOK_OVER.store(true, Ordering::SeqCst);
    crate::node::output::spawn(async move {
        signals.wait().await;
        say("node", "received stop signal");
        stop();
    })
}
