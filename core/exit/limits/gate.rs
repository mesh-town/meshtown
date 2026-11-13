//! The counter of live exit streams: a cap, a slot guard and the reasons to refuse.

use portable_atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Instant;

use super::line_rate::KeyedLineRate;
use super::rss::RssGate;
use super::share::Share;
use super::window::StreamWindow;
use crate::exit::load::{LoadSnapshot, Meter, Outcome, Reason};
use crate::node::machine_memory::MemoryDefaults;
use crate::transport::net::PeerId;

/// Why a new stream is refused.
#[derive(Debug)]
pub(crate) enum Busy {
    Streams { live: u64, limit: u64 },
    Memory { rss_kib: u64, ceiling_kib: u64 },
    Share { held: u64, share: u64 },
}

impl Busy {
    fn reason(&self) -> Reason {
        match self {
            Busy::Streams { .. } => Reason::Limit,
            Busy::Share { .. } => Reason::Share,
            Busy::Memory { .. } => Reason::Memory,
        }
    }

    pub(crate) fn line(&self) -> String {
        match self {
            Busy::Streams { live, limit } => {
                format!("busy: {live} streams open, limit {limit}")
            }
            Busy::Share { held, share } => {
                format!("busy: member holds {held} streams, share {share}")
            }
            Busy::Memory {
                rss_kib,
                ceiling_kib,
            } => format!(
                "busy: memory {} MiB, ceiling {} MiB",
                rss_kib / 1024,
                ceiling_kib / 1024
            ),
        }
    }
}

// Read by the state snapshot in a later step.
#[allow(dead_code)]
/// What the owner's page shows about the load of the exit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LoadView {
    /// Streams open now.
    pub(crate) streams: u64,
    /// The cap in force.
    pub(crate) limit: u64,
    /// The cap by machine memory (what the setting 0 gives).
    pub(crate) limit_auto: u64,
    /// Overload reason, ten-minute totals and members with refusals.
    pub(crate) counts: LoadSnapshot,
}

struct Inner {
    live: AtomicU64,
    limit: AtomicU64,
    auto: AtomicU64,
    rss: RssGate,
    lines: KeyedLineRate<PeerId>,
    window: StreamWindow,
    share: Share,
    load: Mutex<Meter>,
}

/// Live exit streams of the process and the rules for admitting a new one.
#[derive(Clone)]
pub struct ExitLimits(Arc<Inner>);

/// A held place among the live streams; dropping it frees the place.
#[derive(Debug)]
pub(crate) struct StreamSlot(ExitLimits, PeerId);

impl std::fmt::Debug for ExitLimits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExitLimits")
            .field("live", &self.0.live.load(Ordering::Relaxed))
            .field("limit", &self.0.limit.load(Ordering::Relaxed))
            .finish()
    }
}

impl Drop for StreamSlot {
    fn drop(&mut self) {
        self.0 .0.live.fetch_sub(1, Ordering::Relaxed);
        self.0 .0.share.release(&self.1);
    }
}

impl ExitLimits {
    fn build(limit: u64, rss: RssGate) -> Self {
        Self(Arc::new(Inner {
            live: AtomicU64::new(0),
            limit: AtomicU64::new(limit),
            auto: AtomicU64::new(limit),
            rss,
            lines: KeyedLineRate::default(),
            window: StreamWindow::default(),
            share: Share::default(),
            load: Mutex::new(Meter::new(Instant::now())),
        }))
    }

    /// The counter shared by every network of the process.
    pub fn process() -> ExitLimits {
        static PROCESS: OnceLock<ExitLimits> = OnceLock::new();
        PROCESS
            .get_or_init(|| Self::build(MemoryDefaults::LARGE.max_streams, RssGate::process()))
            .clone()
    }

    /// Called once at process start. `setting` 0 means the automatic cap `auto`.
    pub(crate) fn configure(&self, setting: u64, auto: u64, rss_ceiling_kib: u64) {
        self.0.auto.store(auto, Ordering::Relaxed);
        self.0.rss.set_ceiling(rss_ceiling_kib);
        self.set_max_streams(setting);
    }

    /// Changes the cap on the fly; open streams are not touched.
    pub(crate) fn set_max_streams(&self, setting: u64) {
        let limit = match setting {
            0 => self.0.auto.load(Ordering::Relaxed),
            n => n,
        };
        self.0.limit.store(limit, Ordering::Relaxed);
    }

    pub(crate) fn max_streams(&self) -> u64 {
        self.0.limit.load(Ordering::Relaxed)
    }

    /// Called once at process start. `setting_kib` 0 means the tier default `tier_kib`; 0 there means no window.
    pub(crate) fn configure_window(&self, setting_kib: u64, tier_kib: u64) {
        self.0.window.configure(setting_kib, tier_kib);
    }

    /// Changes the stream window on the fly; open streams keep the window they started with.
    pub(crate) fn set_stream_window(&self, setting_kib: u64) {
        self.0.window.set(setting_kib);
    }

    /// The cap on unacknowledged bytes toward the client for a stream being opened; 0 means none.
    pub(crate) fn stream_window_bytes(&self) -> usize {
        self.0.window.bytes()
    }

    /// Takes a place for a new stream: the member's share, the cap, then the process memory.
    /// Every decision is counted for the load meter.
    pub(crate) fn try_admit(&self, peer: PeerId, now: Instant) -> Result<StreamSlot, Busy> {
        let answer = self.admit_checked(peer, now);
        let outcome = match &answer {
            Ok(_) => Outcome::Admitted,
            Err(busy) => Outcome::Refused(busy.reason()),
        };
        self.meter().record(peer, outcome, now);
        answer
    }

    fn admit_checked(&self, peer: PeerId, now: Instant) -> Result<StreamSlot, Busy> {
        let limit = self.max_streams();
        self.0
            .share
            .admit(peer, now, limit, || self.take_place(limit))?;
        let slot = StreamSlot(self.clone(), peer);
        match self.0.rss.over(Instant::now()) {
            Some((rss_kib, ceiling_kib)) => {
                drop(slot);
                self.0.share.note_refused(peer, now);
                Err(Busy::Memory {
                    rss_kib,
                    ceiling_kib,
                })
            }
            None => Ok(slot),
        }
    }

    fn meter(&self) -> MutexGuard<'_, Meter> {
        self.0.load.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Why the exit is overloaded at `now`, or `None` while it is not.
    pub(crate) fn overloaded(&self, now: Instant) -> Option<Reason> {
        self.meter().overloaded(now)
    }

    #[allow(dead_code)]
    /// Numbers for the owner's page at `now`: streams, caps and the meter's ten-minute counts.
    pub(crate) fn load_view(&self, now: Instant) -> LoadView {
        LoadView {
            streams: self.0.live.load(Ordering::Relaxed),
            limit: self.max_streams(),
            limit_auto: self.0.auto.load(Ordering::Relaxed),
            counts: self.meter().snapshot(now),
        }
    }

    fn take_place(&self, limit: u64) -> Result<(), Busy> {
        let taken = self
            .0
            .live
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                (n < limit).then_some(n + 1)
            });
        taken
            .map(|_| ())
            .map_err(|live| Busy::Streams { live, limit })
    }

    /// `Some(refusals of this member held back since its last line)` when a log line is due, else `None`
    /// (the refusal is counted for the next line of this member).
    pub(crate) fn busy_line_due(&self, peer: PeerId, now: Instant) -> Option<u64> {
        self.0.lines.due(peer, now)
    }
}

#[cfg(test)]
impl ExitLimits {
    /// A counter with a fixed cap and no memory guard.
    pub(crate) fn fixed(max_streams: u64) -> Self {
        Self::build(max_streams, RssGate::new(0, || 0))
    }

    pub(crate) fn live(&self) -> u64 {
        self.0.live.load(Ordering::Relaxed)
    }

    pub(crate) fn share_len(&self, now: Instant) -> usize {
        self.0.share.len(now)
    }

    pub(crate) fn unlimited() -> Self {
        Self::fixed(u64::MAX)
    }

    pub(crate) fn with_rss(max_streams: u64, ceiling_kib: u64, read: fn() -> u64) -> Self {
        Self::build(max_streams, RssGate::new(ceiling_kib, read))
    }
}

#[cfg(test)]
#[path = "gate_tests.rs"]
mod tests;
