//! A `tracing` layer for tests: collects the process's events (level, target, message) and counts
//! `WARN` and `ERROR`, so a test can say "no errors have appeared in the log since the `mark()` mark".
//!
//! Why: node lines (`say`) go to stdout via `println!` and do not land here; what lands here is
//! only `tracing` events — `sync/replica/*`, `sync/flood/topic.rs`, `net_hyperdht/*` and
//! dependencies (townjump). They are rare, so the layer is insurance against a silent error, not the main
//! check.
//! Limits: the layer is installed once per process (`OnceLock`, a global subscriber). Under
//! `cargo test`, all tests of one binary share one process and their events mix —
//! `mark()` cuts off only the old ones by time, but not other parallel tests. `nextest` runs
//! each test in its own process, so nothing mixes there. It is plugged in with the line
//! `#[path = "common/log_capture.rs"] mod log_capture;`. The file has no test of its own — the layer is
//! checked by `core/tests/log_capture_check.rs`.

#![allow(dead_code)]

use std::fmt::Debug;
use std::sync::{Mutex, OnceLock};

use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

/// One event: level, target (module) and message.
#[derive(Clone, Debug)]
pub(crate) struct Captured {
    pub(crate) level: Level,
    pub(crate) target: String,
    pub(crate) message: String,
}

/// A mark in the event stream: events before it are "old".
#[derive(Clone, Copy, Debug)]
pub(crate) struct Mark(usize);

static EVENTS: Mutex<Vec<Captured>> = Mutex::new(Vec::new());
static INSTALLED: OnceLock<bool> = OnceLock::new();

struct Capture;

struct MessageOf(String);

impl Visit for MessageOf {
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        if field.name() == "message" {
            self.0 = format!("{value:?}");
        }
    }
}

impl<S: Subscriber> Layer<S> for Capture {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut message = MessageOf(String::new());
        event.record(&mut message);
        let meta = event.metadata();
        EVENTS.lock().unwrap().push(Captured {
            level: *meta.level(),
            target: meta.target().to_string(),
            message: message.0,
        });
    }
}

/// Installs the layer once per process; a repeated call does nothing. Returns whether the layer was installed
/// (`false` — the global subscriber in the process is already someone else's).
pub(crate) fn install() -> bool {
    *INSTALLED.get_or_init(|| {
        tracing_subscriber::registry()
            .with(Capture)
            .try_init()
            .is_ok()
    })
}

/// The "now" mark: installs the layer if it is not there yet.
pub(crate) fn mark() -> Mark {
    install();
    Mark(EVENTS.lock().unwrap().len())
}

/// Events since the mark.
pub(crate) fn since(mark: Mark) -> Vec<Captured> {
    EVENTS.lock().unwrap()[mark.0..].to_vec()
}

/// How many events of level `level` have appeared since the mark.
pub(crate) fn count_since(mark: Mark, level: Level) -> usize {
    since(mark).iter().filter(|e| e.level == level).count()
}

/// How many `WARN`s since the mark.
pub(crate) fn warns_since(mark: Mark) -> usize {
    count_since(mark, Level::WARN)
}

/// Fails if at least one `ERROR` has appeared since the mark; the message holds all such events.
pub(crate) fn assert_no_errors_since(mark: Mark) {
    let errors: Vec<_> = since(mark)
        .into_iter()
        .filter(|e| e.level == Level::ERROR)
        .collect();
    assert!(errors.is_empty(), "ERROR in the log since the mark: {errors:#?}");
}
