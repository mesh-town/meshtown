//! Wake-ups of the node's main loop: which of the seven `select!` branches fired. The branches only
//! hand out `Wake`; handling is in `LoopState::dispatch_wake` (`event_loop.rs`).

use crate::membership::MemberEvent;
use crate::net::PeerAddr;
use tokio::sync::mpsc;
use tokio::time::{Instant, Interval};

use crate::node::dial::{self, DialFailed};

/// What woke the loop.
pub(super) enum Wake {
    Found(PeerAddr),
    Member(MemberEvent),
    Failed(DialFailed),
    RedialDue,
    HelloTick,
    StatusTick,
    PruneTick,
    /// A branch fired, nothing to do (a closed dial failure channel doesn't stop the loop).
    Idle,
    /// The channel of discovered addresses or member events is closed: the loop ends.
    Closed,
}

/// Wake-up sources of the loop: three channels and three timers.
pub(super) struct Wakers {
    pub(super) found_rx: mpsc::Receiver<PeerAddr>,
    pub(super) events_rx: mpsc::UnboundedReceiver<MemberEvent>,
    pub(super) failed_rx: mpsc::UnboundedReceiver<DialFailed>,
    pub(super) hello_interval: Interval,
    pub(super) status_interval: Interval,
    pub(super) prune_interval: Interval,
}

impl Wakers {
    /// Waits for the first branch that fires; `due` is the time of the nearest dial retry.
    pub(super) async fn next(&mut self, due: Option<Instant>) -> Wake {
        tokio::select! {
            addr = self.found_rx.recv() => found_wake(addr),
            event = self.events_rx.recv() => member_wake(event),
            failed = self.failed_rx.recv() => failed_wake(failed),
            _ = dial::sleep_until_due(due) => Wake::RedialDue,
            _ = self.hello_interval.tick() => Wake::HelloTick,
            _ = self.status_interval.tick() => Wake::StatusTick,
            _ = self.prune_interval.tick() => Wake::PruneTick,
        }
    }
}

pub(super) fn found_wake(addr: Option<PeerAddr>) -> Wake {
    addr.map_or(Wake::Closed, Wake::Found)
}

pub(super) fn member_wake(event: Option<MemberEvent>) -> Wake {
    event.map_or(Wake::Closed, Wake::Member)
}

/// A closed failure channel doesn't stop the loop, as before.
pub(super) fn failed_wake(failed: Option<DialFailed>) -> Wake {
    failed.map_or(Wake::Idle, Wake::Failed)
}
