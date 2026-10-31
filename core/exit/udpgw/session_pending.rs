//! Datagram routing of the gateway: a name that is not known yet is looked up in a task of its own, so frames of
//! other `conn_id`s keep flowing; the frames of the same `conn_id` wait in order until the lookup is done.

use std::collections::VecDeque;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::{Gateway, Named};
use crate::exit::outbound::{is_forbidden, is_own_address, udp_address};
use crate::exit::udpgw::codec::{Frame, Target};
use crate::site_checks::doh::Resolver;

/// Frames of one `conn_id` kept while its name is looked up (the first one included); more are dropped.
pub(super) const PENDING_FRAMES: usize = 16;
/// Lookups running at the same time in one stream; a datagram with one more new name is dropped.
pub(super) const MAX_RESOLVING: usize = 32;

/// A `conn_id` waiting for a lookup; the first frame of the queue is the one being looked up.
pub(super) struct Pending {
    id: u64,
    queue: VecDeque<Frame>,
    task: JoinHandle<()>,
}

impl Drop for Pending {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// The end of a lookup: the address and whether it bypasses the address guard, `None` if the name did not resolve.
pub(super) struct Resolved {
    conn_id: u16,
    id: u64,
    found: Option<(SocketAddr, bool)>,
}

enum Route {
    Drop,
    Send(SocketAddr, bool),
    Lookup(String, u16),
}

impl Gateway {
    pub(super) async fn datagram(&mut self, frame: Frame) {
        if let Some(pending) = self.pending.get_mut(&frame.conn_id) {
            return pending.enqueue(frame);
        }
        self.advance(frame, VecDeque::new()).await;
    }

    /// Handles `first`, then the frames behind it, until one of them needs a lookup.
    async fn advance(&mut self, first: Frame, mut rest: VecDeque<Frame>) {
        let mut next = Some(first);
        while let Some(frame) = next {
            if !self.forward(frame, &mut rest).await {
                return;
            }
            next = rest.pop_front();
        }
    }

    /// Whether the frames behind this one may go on (`false`: a lookup took them over).
    async fn forward(&mut self, frame: Frame, rest: &mut VecDeque<Frame>) -> bool {
        match self.route(&frame) {
            Route::Drop => {}
            Route::Send(addr, trusted) => {
                self.send_to(&frame, addr, trusted).await;
            }
            Route::Lookup(name, port) => {
                self.start_lookup(frame, std::mem::take(rest), name, port);
                return false;
            }
        }
        true
    }

    fn route(&self, frame: &Frame) -> Route {
        match &frame.target {
            None => Route::Drop,
            Some(Target::Ip(addr)) => Route::Send(*addr, false),
            Some(Target::Domain(name, port)) => self.route_name(frame.conn_id, name, *port),
        }
    }

    fn route_name(&self, conn_id: u16, name: &str, port: u16) -> Route {
        if let Some(addr) = self.cfg.host_overrides.get(name) {
            return Route::Send(*addr, true);
        }
        if let Ok(ip) = name.parse::<IpAddr>() {
            return Route::Send(SocketAddr::new(ip, port), false);
        }
        if let Some(addr) = self.remembered(conn_id, name, port) {
            return Route::Send(addr, true);
        }
        Route::Lookup(name.to_string(), port)
    }

    /// The address the live session of `conn_id` last got for this very name, already passed by the guard.
    fn remembered(&self, conn_id: u16, name: &str, port: u16) -> Option<SocketAddr> {
        let named = self.sessions.get(&conn_id)?.named.as_ref()?;
        (named.name == name && named.port == port).then_some(named.addr)
    }

    fn start_lookup(&mut self, frame: Frame, rest: VecDeque<Frame>, name: String, port: u16) {
        if self.pending.len() >= MAX_RESOLVING {
            return;
        }
        self.next_lookup += 1;
        let (conn_id, id) = (frame.conn_id, self.next_lookup);
        let (resolver, results) = (self.cfg.resolver.clone(), self.results.clone());
        let task = crate::node::output::spawn(look_up(resolver, results, conn_id, id, name, port));
        let mut queue = rest;
        queue.push_front(frame);
        self.pending.insert(conn_id, Pending { id, queue, task });
    }

    /// Takes the end of a lookup: sends the waiting frames in order, or drops them all.
    pub(super) async fn resolved(&mut self, done: Resolved) {
        let current = self.pending.get(&done.conn_id).map(|p| p.id);
        if current != Some(done.id) {
            return;
        }
        let Some(mut pending) = self.pending.remove(&done.conn_id) else {
            return;
        };
        let (Some((addr, trusted)), Some(head)) = (done.found, pending.queue.pop_front()) else {
            return;
        };
        if !self.send_to(&head, addr, trusted).await {
            return;
        }
        self.remember(&head, addr);
        let mut rest = std::mem::take(&mut pending.queue);
        if let Some(next) = rest.pop_front() {
            self.advance(next, rest).await;
        }
    }

    fn remember(&mut self, frame: &Frame, addr: SocketAddr) {
        let Some(Target::Domain(name, port)) = &frame.target else {
            return;
        };
        let Some(session) = self.sessions.get_mut(&frame.conn_id) else {
            return;
        };
        let (name, port) = (name.clone(), *port);
        session.named = Some(Named { name, port, addr });
    }

    /// Sends the datagram; `false` if the address is closed to the exit or no session could carry it.
    async fn send_to(&mut self, frame: &Frame, addr: SocketAddr, trusted: bool) -> bool {
        if !self.admit(frame.conn_id, addr, trusted) {
            return false;
        }
        let Some(session) = self.session(frame.conn_id, addr) else {
            return false;
        };
        session.touch();
        let _ = session.sock.send_to(&frame.data, addr).await;
        true
    }

    fn admit(&mut self, conn_id: u16, addr: SocketAddr, trusted: bool) -> bool {
        if trusted || self.allowed.contains(&addr) {
            return true;
        }
        if is_forbidden(addr.ip()) || is_own_address(addr.ip()) {
            self.report_refused(conn_id);
            return false;
        }
        self.allowed.insert(addr);
        true
    }
}

impl Pending {
    fn enqueue(&mut self, frame: Frame) {
        if self.queue.len() < PENDING_FRAMES {
            self.queue.push_back(frame);
        }
    }
}

async fn look_up(
    resolver: Arc<Resolver>,
    results: mpsc::Sender<Resolved>,
    conn_id: u16,
    id: u64,
    name: String,
    port: u16,
) {
    let found = lookup(&resolver, &name, port).await;
    let _ = results.send(Resolved { conn_id, id, found }).await;
}

async fn lookup(resolver: &Resolver, name: &str, port: u16) -> Option<(SocketAddr, bool)> {
    #[cfg(test)]
    if let Some(answer) = super::resolve_tests::answer(name).await {
        return answer;
    }
    Some((udp_address(resolver, name, port).await?, false))
}

#[cfg(test)]
#[path = "session_pending_tests.rs"]
mod tests;
