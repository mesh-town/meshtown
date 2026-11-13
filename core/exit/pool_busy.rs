//! What the pool keeps about exits that are busy: the pace of the "busy" log lines and the client's count
//! of refusals per exit.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::limits::LineRate;
use super::load::{Answer, ExitAnswers};
use crate::transport::net::PeerId;

/// Paces the client's "exit is busy" log lines and counts the answers of each exit.
#[derive(Clone)]
pub(super) struct PoolBusy {
    lines: Arc<LineRate>,
    origin: Instant,
    answers: Arc<Mutex<HashMap<PeerId, ExitAnswers>>>,
}

impl PoolBusy {
    pub(super) fn new() -> Self {
        Self {
            lines: Arc::default(),
            origin: Instant::now(),
            answers: Arc::default(),
        }
    }

    pub(super) fn line_due(&self, now: Instant) -> Option<u64> {
        self.lines.due(now)
    }

    pub(super) fn record(&self, peer: PeerId, answer: Answer, now: Instant) {
        let mut all = self.answers.lock().unwrap_or_else(|e| e.into_inner());
        all.retain(|_, a| !a.is_idle(now));
        all.entry(peer)
            .or_insert_with(|| ExitAnswers::new(self.origin))
            .record(answer, now);
    }

    pub(super) fn refusing(&self, peer: PeerId, now: Instant) -> Option<(u64, u64)> {
        let mut all = self.answers.lock().unwrap_or_else(|e| e.into_inner());
        all.get_mut(&peer)?.refusing(now)
    }
}
