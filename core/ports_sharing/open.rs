//! Listeners of other nodes' ports opened here (`Opener`): `127.0.0.1:<local>` → a stream to the publisher.
//!
//! What it does: reconciles the `Opened` list with the running listeners, binds TCP ports synchronously (a UDP port
//! is bound by its task right after the entry is made),
//! hands every accepted connection to `dial::forward_to_peer_port`; closing a listener drops its connections.
//! What it does not do: it does not decide who is a member (`Members`) and does not store the list in the config.
//! A TCP socket lies in a shared cell, not in a task: the port is released before the close returns. A UDP socket
//! is owned by its task and is released shortly after the close, within `UDP_REBIND_WAIT`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::transport::net::{NetStack, PeerId, PORT};
use tokio::net::TcpListener;
use tokio::runtime::Handle;
use tokio::task::AbortHandle;

use super::dial::PortReach;
use super::types::Opened;
use crate::exit::PeerConnPool;
use crate::members::membership::Members;
use crate::node::output::say;

/// Result of binding one `Opened` entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenState {
    /// A UDP socket is not taken yet: a busy port is retried for a short time.
    Binding,
    Listening,
    BindFailed(String),
}

type Slot = Arc<Mutex<Option<TcpListener>>>;
type State = Arc<Mutex<OpenState>>;
type Reach = Arc<Mutex<Option<PortReach>>>;

/// A running listener: a cell with the socket and the accepting task. A UDP listener has no cell: its
/// task owns the socket and releases it when aborted.
pub(crate) struct Listener {
    slot: Option<Slot>,
    task: AbortHandle,
}

impl Listener {
    /// Closes a TCP socket before returning, then aborts the task (and with it all connections).
    /// A UDP socket goes with its aborted task, shortly after.
    fn close(self) {
        if let Some(slot) = &self.slot {
            drop(lock(slot).take());
        }
        self.task.abort();
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

struct Entry {
    opened: Opened,
    state: State,
    /// The result of the last dial: the refusal, or `None` after a success.
    reach: Reach,
    listener: Option<Listener>,
}

impl Entry {
    /// A live listener, bound or still binding; a UDP bind can fail after the entry is made, which the state shows.
    /// A binding entry counts: re-creating it would leave the old task running next to the new one.
    fn is_active(&self) -> bool {
        let live = matches!(
            *lock(&self.state),
            OpenState::Binding | OpenState::Listening
        );
        self.listener.is_some() && live
    }
}

/// Starts the listener of one entry; `None` when the bind failed at once (the state says why).
type BindFn = fn(&Arc<Shared>, Opened, PeerConnPool, State, Reach) -> Option<Listener>;

#[derive(Default)]
struct Inner {
    entries: Vec<Entry>,
    pools: HashMap<PeerId, PeerConnPool>,
    closed: bool,
}

struct Shared {
    stack: NetStack,
    members: Members,
    handle: Handle,
    inner: Mutex<Inner>,
}

impl Drop for Shared {
    fn drop(&mut self) {
        let _rt = self.handle.enter();
        close_inner(&mut lock(&self.inner));
    }
}

/// Listeners of opened ports. Clones share state; when the last clone is gone, everything is closed.
#[derive(Clone)]
pub struct Opener(Arc<Shared>);

impl Opener {
    /// An empty `Opener`; takes `Handle::current()`, panics outside a runtime.
    pub fn new(stack: NetStack, members: Members) -> Opener {
        Opener(Arc::new(Shared {
            stack,
            members,
            handle: Handle::current(),
            inner: Mutex::new(Inner::default()),
        }))
    }

    /// Reconciles listeners with the list: all closes first, then all binds.
    pub fn reconcile_listeners(&self, opened: Vec<Opened>) {
        let _rt = self.0.handle.enter();
        let mut inner = lock(&self.0.inner);
        if inner.closed {
            say("port", "cannot open the port: the network has stopped");
            return;
        }
        let listening = listening_of(&inner.entries);
        let (to_close, to_bind) = listener_changes(&listening, &opened);
        let mut old = std::mem::take(&mut inner.entries);
        close_matching(&mut old, &to_close);
        let entries = self.open_added(&mut inner, &opened, old, &to_bind);
        inner
            .pools
            .retain(|id, _| entries.iter().any(|e| e.opened.node == *id));
        inner.entries = entries;
    }

    /// The new entry list in the order of `opened`: a still-listening entry is kept, every other one is bound.
    fn open_added(
        &self,
        inner: &mut Inner,
        opened: &[Opened],
        mut old: Vec<Entry>,
        to_bind: &[Opened],
    ) -> Vec<Entry> {
        let mut entries = Vec::new();
        for o in dedup(opened) {
            let kept = old.iter().position(|e| e.opened == o && e.is_active());
            entries.push(match kept {
                Some(i) if !to_bind.contains(&o) => old.remove(i),
                _ => self.bind_entry(inner, o),
            });
        }
        entries
    }

    fn bind_entry(&self, inner: &mut Inner, o: Opened) -> Entry {
        let pool = inner
            .pools
            .entry(o.node)
            .or_insert_with(|| PeerConnPool::new(PORT))
            .clone();
        let state: State = Arc::new(Mutex::new(OpenState::Listening));
        let reach: Reach = Arc::new(Mutex::new(None));
        let bind: BindFn = o.proto.pick(bind_tcp_entry, udp::bind_udp_entry);
        let listener = bind(&self.0, o.clone(), pool, state.clone(), reach.clone());
        Entry {
            opened: o,
            state,
            reach,
            listener,
        }
    }

    /// Snapshot of the bind results in the order of the last reconcile.
    pub fn bind_states(&self) -> Vec<(Opened, OpenState, Option<PortReach>)> {
        let inner = lock(&self.0.inner);
        inner
            .entries
            .iter()
            .map(|e| (e.opened.clone(), lock(&e.state).clone(), *lock(&e.reach)))
            .collect()
    }

    /// Final close: the ports are free on return, after that `reconcile_listeners` opens nothing.
    pub fn close_all(&self) {
        let _rt = self.0.handle.enter();
        close_inner(&mut lock(&self.0.inner));
    }
}

fn listening_of(entries: &[Entry]) -> Vec<Opened> {
    entries
        .iter()
        .filter(|e| e.is_active())
        .map(|e| e.opened.clone())
        .collect()
}

fn close_matching(entries: &mut [Entry], close: &[Opened]) {
    for e in entries.iter_mut().filter(|e| close.contains(&e.opened)) {
        close_entry(e);
    }
}

fn close_entry(e: &mut Entry) {
    let was_listening = e.listener.is_some() && *lock(&e.state) == OpenState::Listening;
    if let Some(l) = e.listener.take() {
        l.close();
    }
    if was_listening {
        let suffix = e.opened.proto.pick("", "/udp");
        say(
            "port",
            format!("127.0.0.1:{}{suffix}: closed", e.opened.local),
        );
    }
}

fn close_inner(inner: &mut Inner) {
    inner.closed = true;
    inner.entries.iter_mut().for_each(close_entry);
    inner.entries.clear();
    inner.pools.clear();
}

fn dedup(v: &[Opened]) -> Vec<Opened> {
    let mut out: Vec<Opened> = Vec::new();
    for o in v {
        if !out.contains(o) {
            out.push(o.clone());
        }
    }
    out
}

/// What to close (listening but not needed) and what to bind (needed but not listening); duplicates are collapsed.
pub(crate) fn listener_changes(
    listening: &[Opened],
    wanted: &[Opened],
) -> (Vec<Opened>, Vec<Opened>) {
    let wanted = dedup(wanted);
    let to_close = listening
        .iter()
        .filter(|o| !wanted.contains(o))
        .cloned()
        .collect();
    let to_bind = wanted
        .into_iter()
        .filter(|o| !listening.contains(o))
        .collect();
    (to_close, to_bind)
}

mod listen;
mod udp;
use listen::bind_tcp_entry;
#[cfg(test)]
use listen::spawn_listener;

#[cfg(test)]
#[path = "open_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "open_udp_tests.rs"]
mod udp_tests;

#[cfg(test)]
#[path = "open_udp_binding_tests.rs"]
mod udp_binding_tests;

#[cfg(test)]
#[path = "open_reach_tests.rs"]
mod reach_tests;
