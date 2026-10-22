//! The spinner of a starting node: which frame the grey ring shows and the ticker thread that advances it.
//!
//! What it does: `Spin` remembers the sector count of the last running ring and the frame number, and starts
//! a `Ticker` (thread `tray-spin`) when the node enters `Starting`; the ticker sends `TrayCmd::SpinTick` with
//! its generation. What it does not do: it draws nothing (`menu_build.rs`) and never touches the tray.
//! Links: owned by `Core` (`core.rs`); the frame picture is `app::view_picture::IconPicture::with_idle`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::core::{CmdTx, TrayCmd};
use crate::app::view::{NodeState, View};
use crate::app::view_picture::{spinning, IdleRing};

/// The period between spinner frames: the only place of the tempo.
pub(super) const SPIN_TICK: Duration = Duration::from_millis(100);

/// The flags of one ticker: `alive` is cleared when the node leaves `Starting`; `pending` is set when a tick is
/// in the channel and cleared when `Core` takes it, so the channel never holds more than one tick.
struct TickerFlags {
    alive: AtomicBool,
    pending: AtomicBool,
}

/// What the tray remembers for the idle ring and the spinner frame.
pub(super) struct Spin {
    remembered: u16,
    frame: u32,
    generation: u64,
    ticker: Option<Arc<TickerFlags>>,
    period: Duration,
}

/// A ticker that is created but not yet running: `spawn` starts its thread.
pub(super) struct Ticker {
    flags: Arc<TickerFlags>,
    generation: u64,
    period: Duration,
}

impl Spin {
    pub(super) fn new() -> Spin {
        Spin { remembered: 0, frame: 0, generation: 0, ticker: None, period: SPIN_TICK }
    }

    /// A spinner with a custom tick period (for tests).
    #[cfg(test)]
    pub(super) fn with_period(period: Duration) -> Spin {
        Spin { period, ..Spin::new() }
    }

    /// Follows the view: remembers the running ring size; a spinning view (`spinning`) returns a new ticker
    /// to spawn, any other view stops the live one.
    pub(super) fn follow(&mut self, view: View) -> Option<Ticker> {
        self.remember(view);
        if !spinning(view) {
            self.stop_ticker();
            return None;
        }
        if self.ticker.is_some() {
            return None;
        }
        Some(self.start_ticker())
    }

    /// Takes a tick: `true` (and the next frame) only for a live ticker of the current generation.
    pub(super) fn tick(&mut self, generation: u64) -> bool {
        let Some(flags) = &self.ticker else { return false };
        if generation != self.generation {
            return false;
        }
        flags.pending.store(false, Ordering::SeqCst);
        self.frame = self.frame.wrapping_add(1);
        true
    }

    /// The memory for `IconPicture::with_idle`.
    pub(super) fn idle(&self) -> IdleRing {
        IdleRing { remembered: self.remembered, frame: self.frame }
    }

    #[cfg(test)]
    pub(super) fn generation(&self) -> u64 {
        self.generation
    }

    fn remember(&mut self, view: View) {
        if view.node != NodeState::Running || view.ring.is_empty() {
            return;
        }
        self.remembered = u16::try_from(view.ring.len()).unwrap_or(u16::MAX);
    }

    fn start_ticker(&mut self) -> Ticker {
        self.generation += 1;
        self.frame = 0;
        let flags = Arc::new(TickerFlags { alive: AtomicBool::new(true), pending: AtomicBool::new(false) });
        self.ticker = Some(flags.clone());
        Ticker { flags, generation: self.generation, period: self.period }
    }

    fn stop_ticker(&mut self) {
        if let Some(flags) = self.ticker.take() {
            flags.alive.store(false, Ordering::SeqCst);
        }
    }
}

impl Ticker {
    /// Starts the thread `tray-spin`: wait a period, then send one tick, until `alive` is cleared. A failed
    /// spawn is logged; the ring then stands on its first frame.
    pub(super) fn spawn(self, tx: CmdTx) {
        let Ticker { flags, generation, period } = self;
        let run = move || loop {
            std::thread::sleep(period);
            let mut send = |g| tx.send(TrayCmd::SpinTick(g));
            if !ticker_step(&flags, generation, &mut send) {
                break;
            }
        };
        if let Err(e) = std::thread::Builder::new().name("tray-spin".into()).spawn(run) {
            super::log_tray(&format!("spinner thread did not start: {e}"));
        }
    }

    #[cfg(test)]
    pub(super) fn is_alive(&self) -> bool {
        self.flags.alive.load(Ordering::SeqCst)
    }
}

/// One ticker period: `false` when the ticker is dead (no send), otherwise sends the tick unless the previous
/// one is still unread, and returns `true`.
fn ticker_step(flags: &TickerFlags, generation: u64, send: &mut dyn FnMut(u64)) -> bool {
    if !flags.alive.load(Ordering::SeqCst) {
        return false;
    }
    if flags.pending.swap(true, Ordering::SeqCst) {
        return true;
    }
    send(generation);
    true
}

#[cfg(test)]
#[path = "spin_tests.rs"]
mod tests;
