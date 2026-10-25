//! Direct-dial addresses of peers: hints given by invites and the cache, and the address that last worked.

use crate::transport::net::addr::{AddrHint, PeerAddr};
use crate::transport::net::id::PeerId;
use std::collections::HashMap;
use std::net::{SocketAddr, SocketAddrV4};
use std::sync::{Mutex, MutexGuard};

/// How many peers the book remembers; the one touched longest ago is dropped first.
pub(super) const MAX_HINT_PEERS: usize = 256;
/// How many hints are kept per peer, the newest first.
pub(super) const MAX_HINTS_PER_PEER: usize = 2;
/// How many addresses one dial tries directly.
pub(super) const MAX_DIRECT_TRIES: usize = 2;

#[derive(Default)]
struct Entry {
    hints: Vec<SocketAddrV4>,
    proven: Option<SocketAddrV4>,
    touched: u64,
}

#[derive(Default)]
struct Book {
    peers: HashMap<PeerId, Entry>,
    clock: u64,
}

impl Book {
    fn entry(&mut self, peer: PeerId) -> &mut Entry {
        self.clock += 1;
        let clock = self.clock;
        self.make_room(peer);
        let entry = self.peers.entry(peer).or_default();
        entry.touched = clock;
        entry
    }

    fn make_room(&mut self, peer: PeerId) {
        if self.peers.contains_key(&peer) || self.peers.len() < MAX_HINT_PEERS {
            return;
        }
        let oldest = self.peers.iter().min_by_key(|(_, e)| e.touched);
        let oldest = oldest.map(|(id, _)| *id);
        oldest.into_iter().for_each(|id| {
            self.peers.remove(&id);
        });
    }
}

/// Hints and proven addresses per peer. Only IPv4 `Ip` hints are kept: that is what a direct dial takes.
#[derive(Default)]
pub(super) struct HintBook {
    book: Mutex<Book>,
}

fn v4(hint: &AddrHint) -> Option<SocketAddrV4> {
    match hint {
        AddrHint::Ip(SocketAddr::V4(sa)) => Some(*sa),
        _ => None,
    }
}

impl HintBook {
    fn lock(&self) -> MutexGuard<'_, Book> {
        self.book.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Remembers the IPv4 hints of `addr`; a duplicate moves to the front.
    pub(super) fn note(&self, addr: &PeerAddr) {
        let found: Vec<SocketAddrV4> = addr.hints.iter().filter_map(v4).collect();
        if found.is_empty() {
            return;
        }
        let mut book = self.lock();
        let entry = book.entry(addr.id);
        for at in found {
            entry.hints.retain(|known| *known != at);
            entry.hints.insert(0, at);
        }
        entry.hints.truncate(MAX_HINTS_PER_PEER);
    }

    /// Addresses to dial directly: the proven one first, then the hints, no repeats.
    pub(super) fn targets(&self, peer: PeerId) -> Vec<SocketAddrV4> {
        let book = self.lock();
        let Some(entry) = book.peers.get(&peer) else {
            return Vec::new();
        };
        let mut out: Vec<SocketAddrV4> = entry.proven.into_iter().collect();
        for at in &entry.hints {
            if !out.contains(at) {
                out.push(*at);
            }
        }
        out.truncate(MAX_DIRECT_TRIES);
        out
    }

    /// `at` answered a direct dial with the right key.
    pub(super) fn prove(&self, peer: PeerId, at: SocketAddrV4) {
        self.lock().entry(peer).proven = Some(at);
    }

    /// A direct dial of `at` failed: it stops being proven but stays a hint.
    pub(super) fn disprove(&self, peer: PeerId, at: SocketAddrV4) {
        let mut book = self.lock();
        let Some(entry) = book.peers.get_mut(&peer) else {
            return;
        };
        if entry.proven == Some(at) {
            entry.proven = None;
        }
    }

    pub(super) fn proven(&self, peer: PeerId) -> Option<SocketAddrV4> {
        self.lock().peers.get(&peer).and_then(|e| e.proven)
    }
}

#[cfg(test)]
#[path = "hints_tests.rs"]
mod tests;
