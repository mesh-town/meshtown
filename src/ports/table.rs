//! Table of our own publications: the list, the local check status of each port and a "the list changed"
//! signal for the announcer. The methods are synchronous; the lock is not carried across `.await`.

use std::sync::{Arc, Mutex};

use tokio::sync::Notify;

use super::record::PortRecord;
use super::types::{Audience, Published};

/// Result of the last check of the port on this device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalState {
    /// Not checked.
    Unknown,
    /// Responds.
    Ok,
    /// Does not respond.
    Refused,
}

type Rows = Vec<(Published, LocalState)>;

/// Shared state of the table behind an `Arc`.
#[derive(Clone)]
pub struct PublishedPortsTable {
    rows: Arc<Mutex<Rows>>,
    changed: Arc<Notify>,
}

/// Removes port duplicates (the first one wins), all statuses are `Unknown`.
fn fresh_rows(list: Vec<Published>) -> Rows {
    let mut rows: Rows = Vec::new();
    for p in list {
        if rows.iter().all(|(q, _)| q.port != p.port) {
            rows.push((p, LocalState::Unknown));
        }
    }
    rows
}

/// The status carries over only if the old row has the same port and the entry is entirely equal.
fn kept_state(old: &Rows, p: &Published) -> LocalState {
    match old.iter().find(|(q, _)| q.port == p.port) {
        Some((q, s)) if PortRecord::from(q) == PortRecord::from(p) => *s,
        _ => LocalState::Unknown,
    }
}

impl PublishedPortsTable {
    /// A table from a publication list, all statuses are `Unknown`; does not wake `changed`.
    pub fn new(published: Vec<Published>) -> PublishedPortsTable {
        PublishedPortsTable {
            rows: Arc::new(Mutex::new(fresh_rows(published))),
            changed: Arc::new(Notify::new()),
        }
    }

    /// Replaces the list and always wakes `changed` (the permit accumulates, a change is not lost).
    pub fn set_published(&self, published: Vec<Published>) {
        let mut rows = fresh_rows(published);
        let mut guard = self.rows.lock().unwrap();
        for (p, s) in rows.iter_mut() {
            *s = kept_state(&guard, p);
        }
        *guard = rows;
        drop(guard);
        self.changed.notify_one();
    }

    /// The recipient circle of a port; `None` if the port is not published.
    pub fn audience_of(&self, port: u16) -> Option<Audience> {
        let rows = self.rows.lock().unwrap();
        rows.iter()
            .find(|(p, _)| p.port == port)
            .map(|(p, _)| p.audience.clone())
    }

    /// Records the result of a port check; if there is no such port (removed in a race), nothing, `changed` is not woken.
    pub fn set_local_state(&self, port: u16, state: LocalState) {
        let mut rows = self.rows.lock().unwrap();
        if let Some(row) = rows.iter_mut().find(|(p, _)| p.port == port) {
            row.1 = state;
        }
    }

    /// Rows in the order of the last `new`/`set_published`.
    pub fn published_with_state(&self) -> Vec<(Published, LocalState)> {
        self.rows.lock().unwrap().clone()
    }

    /// The list change signal; there is a single waiter — the announcer.
    pub fn changed_signal(&self) -> Arc<Notify> {
        self.changed.clone()
    }
}

#[cfg(test)]
#[path = "table_tests.rs"]
mod tests;
