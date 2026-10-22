//! The "Quit" signal with a 15 s watchdog and a hard process exit.
//!
//! What it does: `QuitSignal` — a cloneable "quit requested" flag on `tokio::sync::watch`; the first
//! `request()` atomically raises a watchdog in a separate thread (not through `tokio::spawn`/`Handle`,
//! so that `QuitSignal::new()` doesn't require an "entered" runtime on the calling thread): if the node
//! hasn't stopped within `QUIT_WATCHDOG`, the watchdog calls `exit_now(1)`. `exit_now` flushes stdout and
//! stderr, then a hard process exit (on Linux the icon's `Drop` waits for the ksni thread, so
//! quitting the program is done only this way).
//! What it doesn't do: it knows nothing about the node, the page or the tray — only a flag and a timer.
//! Links: `wait()` is called by the `tray` process poller, `request()` by the handler of the "Quit"
//! menu item (`tray`); tests poke the private `mark_requested()` rather than `request()` — otherwise the
//! 15-second watchdog could kill the `cargo test` process in the middle of an end-to-end check.

use std::io::Write as _;
use std::time::Duration;

use tokio::sync::watch;

/// The watchdog period: if the node hasn't stopped within 15 s after "Quit" — a hard exit with an error.
pub const QUIT_WATCHDOG: Duration = Duration::from_secs(15);

/// A cloneable "Quit requested" signal.
#[derive(Clone)]
pub struct QuitSignal {
    tx: watch::Sender<bool>,
}

impl QuitSignal {
    pub fn new() -> Self {
        let (tx, _rx) = watch::channel(false);
        QuitSignal { tx }
    }

    /// Requests quit. The first call raises a 15 s watchdog; a repeated one is a no-op (a second
    /// watchdog isn't started).
    pub fn request(&self) {
        if self.mark_requested() {
            std::thread::spawn(|| {
                std::thread::sleep(QUIT_WATCHDOG);
                exit_now(1);
            });
        }
    }

    /// Returns when quit is requested. `watch::Receiver::wait_for` first checks the current
    /// value — a call after `request()`/`mark_requested()` returns immediately.
    pub async fn wait(&self) {
        let mut rx = self.tx.subscribe();
        let _ = rx.wait_for(|requested| *requested).await;
    }

    /// Atomically checks and sets the flag: `true` — this is the first transition (the watchdog must be
    /// raised), `false` — a repeat. `request()` is built on this same logic.
    fn mark_requested(&self) -> bool {
        self.tx.send_if_modified(|requested| {
            if *requested {
                false
            } else {
                *requested = true;
                true
            }
        })
    }
}

impl Default for QuitSignal {
    fn default() -> Self {
        Self::new()
    }
}

/// Flushes stdout/stderr and hard-exits the process. The only place the module exits the process.
pub fn exit_now(code: i32) -> ! {
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    std::process::exit(code);
}

#[cfg(test)]
#[path = "quit_tests.rs"]
mod tests;
