//! Unified node output to stdout.
//!
//! What it does: prints lines like `+<seconds>s [tag] text` with a single timer from the start;
//! keeps the log ring (`LogRing`) for the node page; `boot_id` belongs to the slot (`node_ctl::BootStamp`).
//! What it doesn't do: it doesn't write to stderr and doesn't format errors — tracing does that.
//! Links: used by all node modules for output; `web::NodeControl` reads the log through
//! `log_page_for`/`log_summary`.

use std::sync::{LazyLock, Mutex};
use std::time::Instant;

use crate::transport::net::PeerId;

use crate::web_gui::{LogPage, LogRing, LogSummary};

/// The name of our node network in texts for people (tray, page, CLI, API error texts).
/// The final product name hasn't been chosen yet — the final name
/// must not contain the word VPN. If the name changes, only this constant is edited. It lives here, not in
/// `app::view` (the tray texts), because almost all places that use it are in the library crate
/// `meshtown`, while `app` exists only in the binary crate `meshtown`; the binary crate sees the
/// constant as `crate::node::output::NET_NAME` (in `commands.rs` — by the import
/// `use crate::node::output::{say, NET_NAME};`).
pub const NET_NAME: &str = "Mesh Town";

/// The app name in texts for people (tray). It is separate from `NET_NAME`:
/// this is the program name, not the name of the node network. The binary crate sees the constant as
/// `crate::node::output::APP_NAME` (like `NET_NAME` in `app/view.rs`).
pub const APP_NAME: &str = "Mesh Town";

/// The capacity of the log ring before settings are applied (`config_file.rs`, `log.ring_lines`).
const DEFAULT_RING_CAPACITY: usize = 2000;

/// The process log: a single line timer and a ring for the page. One value per process
/// (`JOURNAL`); the module's free functions are wrappers over it.
pub struct Journal {
    start: Instant,
    ring: Mutex<LogRing>,
}

impl Journal {
    /// A new log; the clock starts here.
    pub fn new(capacity: usize) -> Self {
        Self {
            start: Instant::now(),
            ring: Mutex::new(LogRing::new(capacity)),
        }
    }

    /// Prints a line with a tag and text and puts the same line into the ring.
    pub fn say(&self, tag: &str, text: impl std::fmt::Display) {
        let t = self.uptime_secs();
        println!("+{t:>7.1}s [{tag}] {text}");
        self.ring().push(t, tag, text.to_string());
    }

    /// Seconds since the log was created — the same scale `say` prints.
    pub fn uptime_secs(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    /// A page of lines with `seq > after`, at most `limit`; the caller sets `boot_id`.
    pub fn page(&self, boot_id: &str, after: u64, limit: usize) -> LogPage {
        self.ring().page(boot_id, after, limit)
    }

    /// A summary: tags, bounds, the tail of the last `tail` lines.
    pub fn summary(&self, tail: usize) -> LogSummary {
        self.ring().summary(tail)
    }

    /// Changes the ring capacity; extra old lines are dropped.
    pub fn set_capacity(&self, lines: usize) {
        self.ring().set_capacity(lines);
    }

    fn ring(&self) -> std::sync::MutexGuard<'_, LogRing> {
        self.ring.lock().unwrap_or_else(|e| e.into_inner())
    }
}

static JOURNAL: LazyLock<Journal> = LazyLock::new(|| Journal::new(DEFAULT_RING_CAPACITY));

/// Prints an output line with a tag and text and puts the same line into the log ring.
pub fn say(tag: &str, text: impl std::fmt::Display) {
    JOURNAL.say(tag, text);
}

/// A short representation of the node identifier.
pub fn short_peer_id(id: &PeerId) -> String {
    id.fmt_short().to_string()
}

/// Seconds since the log started — the same time scale `say` prints.
pub fn uptime_secs() -> f64 {
    JOURNAL.uptime_secs()
}

/// A log page for a slot: the caller supplies `boot_id` (`NodeCtl::log_after`).
pub fn log_page_for(boot_id: &str, after: u64, limit: usize) -> LogPage {
    JOURNAL.page(boot_id, after, limit)
}

/// A log page without a slot: `boot_id` is empty. For tests and code outside the node.
pub fn log_page(after: u64, limit: usize) -> LogPage {
    log_page_for("", after, limit)
}

/// A log summary: tags, bounds, the tail of the last `tail` lines.
pub fn log_summary(tail: usize) -> LogSummary {
    JOURNAL.summary(tail)
}

/// Changes the log ring capacity (`log.ring_lines` from the settings).
pub fn set_log_capacity(lines: usize) {
    JOURNAL.set_capacity(lines);
}

#[cfg(test)]
#[path = "output_tests.rs"]
mod tests;
