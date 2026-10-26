//! Listeners of other nodes' ports opened here (`Opener`): `127.0.0.1:<local>` → a stream to the publisher.
//!
//! What it does: reconciles the `Opened` list with the running listeners, binds ports synchronously,
//! hands every accepted connection to `dial::forward_to_peer_port`; closing a listener drops its connections.
//! What it does not do: it does not decide who is a member (`Members`) and does not store the list in the config.
//! The socket lies in a shared cell, not in a task: the port is released before the close returns.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::transport::net::{NetStack, PeerId, PORT};
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Handle;
use tokio::task::AbortHandle;

use super::dial::{forward_to_peer_port, PortReach};
use super::types::Opened;
use crate::exit::PeerConnPool;
use crate::members::membership::Members;
use crate::node::output::say;

/// Result of binding one `Opened` entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenState {
    Listening,
    BindFailed(String),
}

type Slot = Arc<Mutex<Option<TcpListener>>>;
/// A running listener: a cell with the socket and the accepting task.
pub(crate) struct Listener {
    slot: Slot,
    task: AbortHandle,
}

impl Listener {
    /// Closes the socket before returning, then aborts the task (and with it all connections).
    fn close(self) {
        drop(lock(&self.slot).take());
        self.task.abort();
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

struct Entry {
    opened: Opened,
    state: OpenState,
    listener: Option<Listener>,
}

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
            let kept = old
                .iter()
                .position(|e| e.opened == o && e.listener.is_some());
            entries.push(match kept {
                Some(i) if !to_bind.contains(&o) => old.remove(i),
                _ => self.bind_entry(inner, o),
            });
        }
        entries
    }

    fn bind_entry(&self, inner: &mut Inner, o: Opened) -> Entry {
        let bound = bind_loopback(o.local);
        let pool = inner
            .pools
            .entry(o.node)
            .or_insert_with(|| PeerConnPool::new(PORT))
            .clone();
        match bound {
            Ok(l) => {
                say("port", format!("127.0.0.1:{}: opened", o.local));
                let conn = self.forward_handler(pool, o.clone());
                Entry {
                    opened: o,
                    state: OpenState::Listening,
                    listener: Some(spawn_listener(l, conn)),
                }
            }
            Err(text) => {
                say("port", format!("127.0.0.1:{}: not opened: {text}", o.local));
                Entry {
                    opened: o,
                    state: OpenState::BindFailed(text),
                    listener: None,
                }
            }
        }
    }

    fn forward_handler(
        &self,
        pool: PeerConnPool,
        target: Opened,
    ) -> impl Fn(TcpStream) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static
    {
        let (stack, members) = (self.0.stack.clone(), self.0.members.clone());
        move |tcp| {
            let (stack, members, pool, target) =
                (stack.clone(), members.clone(), pool.clone(), target.clone());
            Box::pin(
                async move { forward_to_peer_port(&stack, &pool, &members, &target, tcp).await },
            )
        }
    }

    /// Snapshot of the bind results in the order of the last reconcile.
    pub fn bind_states(&self) -> Vec<(Opened, OpenState, Option<PortReach>)> {
        let inner = lock(&self.0.inner);
        inner
            .entries
            .iter()
            .map(|e| (e.opened.clone(), e.state.clone(), None))
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
        .filter(|e| e.listener.is_some())
        .map(|e| e.opened.clone())
        .collect()
}

fn close_matching(entries: &mut [Entry], close: &[Opened]) {
    for e in entries.iter_mut().filter(|e| close.contains(&e.opened)) {
        close_entry(e);
    }
}

fn close_entry(e: &mut Entry) {
    if let Some(l) = e.listener.take() {
        l.close();
        say("port", format!("127.0.0.1:{}: closed", e.opened.local));
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
use listen::{bind_loopback, spawn_listener};

#[cfg(test)]
#[path = "open_tests.rs"]
mod tests;
