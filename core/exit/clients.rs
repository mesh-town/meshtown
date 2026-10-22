//! Registry of exit clients: which members are currently reaching the internet through this node.
//!
//! What it does: counts successfully opened exit streams per member connection and remembers a
//! member's "last activity"; a client is one that has an open stream or whose stream
//! closed less than `EXIT_CLIENT_LINGER` ago. Guards (RAII) do the bookkeeping: `ConnGuard` per connection and
//! `StreamGuard` per stream, so an entry survives neither a return, nor a panic, nor a task `abort`.
//! What it doesn't do: it doesn't count bytes or time, doesn't check permission (that is `server`), doesn't filter
//! by network members and doesn't sort — the snapshot builder does that (`node_ctl::snapshot::build`).
//! Links: `server::ExitService::handle` creates a `ConnGuard`, `serve_exit_stream` a `StreamGuard`;
//! `node_ctl::snapshot::collect` reads it through `active_peers_at`.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::net::PeerId;

/// How long a member stays in the list after its last stream closes: the page polls
/// the snapshot once a second, and between page loads a browser holds zero streams for 5–20 s — a shorter window
/// would flicker, a window over a minute makes the badge lie about a client that left long ago.
pub const EXIT_CLIENT_LINGER: Duration = Duration::from_secs(30);

/// A member's connection: how many exit streams are open in it.
#[derive(Debug)]
struct Conn {
    peer: PeerId,
    streams: usize,
}

/// State under the lock: connections by their own counter, and members' last activity.
/// The connection token is our own counter, not the connection's `stable_id()`, which can repeat.
#[derive(Default, Debug)]
struct ClientsState {
    next_token: u64,
    conns: HashMap<u64, Conn>,
    last_active: HashMap<PeerId, Instant>,
}

impl ClientsState {
    fn open_conn(&mut self, peer: PeerId) -> u64 {
        self.next_token += 1;
        self.conns.insert(self.next_token, Conn { peer, streams: 0 });
        self.next_token
    }

    /// Removes expired "last activity" entries.
    fn purge(&mut self, now: Instant) {
        self.last_active.retain(|_, at| *at + EXIT_CLIENT_LINGER > now);
    }

    /// `false` if the connection no longer exists.
    fn add_stream(&mut self, token: u64) -> bool {
        let Some(conn) = self.conns.get_mut(&token) else {
            return false;
        };
        conn.streams += 1;
        true
    }

    fn close_stream(&mut self, token: u64, now: Instant) {
        let Some(conn) = self.conns.get_mut(&token) else {
            return;
        };
        conn.streams = conn.streams.saturating_sub(1);
        self.last_active.insert(conn.peer, now);
    }

    /// Forgets the connection together with its open streams; if there were streams, the member
    /// counts as active "now", and the `EXIT_CLIENT_LINGER` window starts from that moment.
    fn close_conn(&mut self, token: u64, now: Instant) {
        let Some(conn) = self.conns.remove(&token) else {
            return;
        };
        if conn.streams > 0 {
            self.last_active.insert(conn.peer, now);
        }
    }
}

/// Registry of exit clients; a cheap `Clone`, shared by the whole node.
#[derive(Clone, Default, Debug)]
pub struct Clients(Arc<Mutex<ClientsState>>);

impl Clients {
    /// Creates a member's connection entry with zero streams. An empty connection does not
    /// count as a client.
    pub fn register_conn(&self, peer: PeerId) -> ConnGuard {
        let token = self.0.lock().unwrap().open_conn(peer);
        ConnGuard {
            conn: ConnRef {
                clients: self.clone(),
                token,
            },
        }
    }

    /// Members with an open stream in any connection or with activity within the last
    /// `EXIT_CLIENT_LINGER`; no duplicates, order unspecified. Deletes expired entries.
    pub fn active_peers_at(&self, now: Instant) -> Vec<PeerId> {
        let mut guard = self.0.lock().unwrap();
        guard.purge(now);
        let mut peers: HashSet<PeerId> = guard.last_active.keys().copied().collect();
        peers.extend(guard.conns.values().filter(|c| c.streams > 0).map(|c| c.peer));
        peers.into_iter().collect()
    }
}

/// Connection guard: lives in `ExitService::handle` for the whole `accept` loop. `Drop` forgets
/// this connection's open streams immediately, without waiting for the `splice` tail.
#[derive(Debug)]
pub struct ConnGuard {
    conn: ConnRef,
}

impl ConnGuard {
    /// Returns a reference to the connection for stream tasks.
    pub fn conn(&self) -> ConnRef {
        self.conn.clone()
    }
}

impl Drop for ConnGuard {
    fn drop(&mut self) {
        let mut guard = self.conn.clients.0.lock().unwrap();
        guard.close_conn(self.conn.token, Instant::now());
    }
}

/// A cheap connection reference: the registry and a token (the member is stored in the connection entry).
#[derive(Clone, Debug)]
pub struct ConnRef {
    clients: Clients,
    token: u64,
}

impl ConnRef {
    /// Counts a successfully opened stream. If the connection no longer exists, the guard is empty and means nothing.
    pub fn register_stream(&self) -> StreamGuard {
        let mut guard = self.clients.0.lock().unwrap();
        guard.purge(Instant::now());
        let live = guard.add_stream(self.token);
        StreamGuard {
            conn: live.then(|| self.clone()),
        }
    }
}

/// Stream guard: `Drop` decrements the connection's stream counter and sets "member's last activity
/// = now"; fires on return, panic and task `abort`.
#[derive(Debug)]
pub struct StreamGuard {
    conn: Option<ConnRef>,
}

impl Drop for StreamGuard {
    fn drop(&mut self) {
        let Some(conn) = &self.conn else {
            return;
        };
        let mut guard = conn.clients.0.lock().unwrap();
        guard.close_stream(conn.token, Instant::now());
    }
}

#[cfg(test)]
#[path = "clients_tests.rs"]
mod tests;
