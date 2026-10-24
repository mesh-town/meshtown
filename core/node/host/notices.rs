//! Notices the host keeps for the page until they are acknowledged: today only "a network was deleted by
//! an administrator". The list is per host, not per slot: the slot of a deleted network is gone.

use std::sync::Mutex;

use serde::Serialize;

use crate::transport::net::PeerId;

/// The most notices kept; a new one beyond it pushes out the oldest.
pub(crate) const MAX_NOTICES: usize = 8;

const NET_DELETED: &str = "net_deleted";

/// One notice as the page gets it in `notices[]`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Notice {
    pub id: u64,
    pub kind: &'static str,
    pub label: Option<String>,
    pub id_hex: Option<String>,
    /// The full id of the administrator, in the form of `peers[].id_full`.
    pub by: String,
    pub by_name: Option<String>,
    pub at_unix: u64,
}

#[derive(Default)]
struct Book {
    next: u64,
    list: Vec<Notice>,
}

/// The notices in memory: ids grow from 1 and are not reused within a run; oldest first.
#[derive(Default)]
pub(crate) struct NoticeBook(Mutex<Book>);

impl NoticeBook {
    /// Adds a "network deleted" notice and returns its id; the same `(id_hex, by, at_unix)` already in
    /// the list returns the id it has.
    pub(crate) fn push_deleted(
        &self,
        label: Option<String>,
        id_hex: Option<String>,
        by: &PeerId,
        by_name: Option<String>,
        at_unix: u64,
    ) -> u64 {
        let by = by.to_string();
        let mut book = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let same = |n: &Notice| n.id_hex == id_hex && n.by == by && n.at_unix == at_unix;
        if let Some(found) = book.list.iter().find(|n| same(n)) {
            return found.id;
        }
        book.next += 1;
        let id = book.next;
        book.list.push(Notice {
            id,
            kind: NET_DELETED,
            label,
            id_hex,
            by,
            by_name,
            at_unix,
        });
        if book.list.len() > MAX_NOTICES {
            book.list.remove(0);
        }
        id
    }

    /// The notices, oldest first.
    pub(crate) fn list(&self) -> Vec<Notice> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .list
            .clone()
    }

    /// Removes the notice; `false` when there is none with this id.
    pub(crate) fn ack(&self, id: u64) -> bool {
        let mut book = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let before = book.list.len();
        book.list.retain(|n| n.id != id);
        book.list.len() != before
    }
}

#[cfg(test)]
#[path = "notices_tests.rs"]
mod tests;
