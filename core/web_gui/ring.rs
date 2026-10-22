//! Log ring: one implementation for the whole web layer (listener, demo, future node). The listener does not
//! hold or call the ring itself — `NodeControl` implementations use it (`api.md`, `LogRing`).
//! There are no locks inside: the owner puts the ring under a `std::sync::Mutex` itself.

use std::collections::{BTreeSet, VecDeque};

use super::{LogLine, LogPage, LogSummary};

pub struct LogRing {
    capacity: usize,
    next_seq: u64,
    lines: VecDeque<LogLine>,
    tags: BTreeSet<String>,
}

impl LogRing {
    pub fn new(capacity: usize) -> LogRing {
        LogRing {
            capacity: capacity.max(1),
            next_seq: 1,
            lines: VecDeque::new(),
            tags: BTreeSet::new(),
        }
    }

    pub fn set_capacity(&mut self, capacity: usize) {
        self.capacity = capacity.max(1);
        while self.lines.len() > self.capacity {
            self.lines.pop_front();
        }
    }

    pub fn push(&mut self, t: f64, tag: &str, text: String) {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.tags.insert(tag.to_string());
        self.lines.push_back(LogLine {
            seq,
            t,
            tag: tag.to_string(),
            text,
        });
        while self.lines.len() > self.capacity {
            self.lines.pop_front();
        }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    fn bounds(&self) -> (u64, u64) {
        match (self.lines.front(), self.lines.back()) {
            (Some(first), Some(last)) => (first.seq, last.seq),
            _ => (0, 0),
        }
    }

    pub fn page(&self, boot_id: &str, after: u64, limit: usize) -> LogPage {
        let (first_seq, last_seq) = self.bounds();
        let lines: Vec<LogLine> = self
            .lines
            .iter()
            .filter(|line| line.seq > after)
            .take(limit)
            .cloned()
            .collect();
        LogPage {
            boot_id: boot_id.to_string(),
            first_seq,
            last_seq,
            lines,
        }
    }

    pub fn summary(&self, tail: usize) -> LogSummary {
        let (first_seq, last_seq) = self.bounds();
        let tail_lines: Vec<LogLine> = self.lines.iter().rev().take(tail).rev().cloned().collect();
        LogSummary {
            tags: self.tags.iter().cloned().collect(),
            ring_lines: self.capacity as u64,
            first_seq,
            last_seq,
            tail: tail_lines,
        }
    }
}

#[cfg(test)]
#[path = "ring_tests.rs"]
mod tests;
