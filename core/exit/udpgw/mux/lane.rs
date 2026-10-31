//! One shared stream to the exit: a writer task, a reader task and the local connections riding on it.
//! When the stream dies for any reason, every local connection of the lane is closed with it.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use portable_atomic::AtomicU64;
use tokio::io::{split, AsyncWriteExt};
use tokio::sync::{mpsc, watch, Notify};
use tokio::task::AbortHandle;
use tokio::time::Instant;

use super::frame::{read_raw, Kind, RawFrame};
use super::ids::{LocalKey, SessionTable, LANE_SESSIONS, SESSION_IDLE};
use crate::transport::net::{BoxRecv, BoxSend};

/// Frames waiting for the stream writer.
pub(super) const LANE_QUEUE: usize = 64;
/// Frames waiting for one local connection; a full queue drops the frame.
pub(super) const LOCAL_QUEUE: usize = 64;
/// Buffer of the in-memory pipe to a local connection.
const LOCAL_PIPE: usize = 64 * 1024;

pub(super) struct Lane {
    table: Mutex<SessionTable>,
    locals: Mutex<HashMap<LocalKey, mpsc::Sender<Vec<u8>>>>,
    out: mpsc::Sender<Vec<u8>>,
    /// Handles of the reader, the writer and the local tasks, aborted when the lane dies.
    tasks: Mutex<Vec<AbortHandle>>,
    dead: AtomicBool,
    ended: watch::Sender<bool>,
    closing: Notify,
    next_key: AtomicU64,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Lane {
    /// Starts the lane on an open stream. The first local connection is registered before the tasks run,
    /// so a stream the exit closes right away still gives it a pair that sees the end.
    pub(super) fn start(send: BoxSend, recv: BoxRecv) -> (Arc<Lane>, (BoxSend, BoxRecv)) {
        let (out, queue) = mpsc::channel(LANE_QUEUE);
        let lane = Arc::new(Lane {
            table: Mutex::new(SessionTable::new(LANE_SESSIONS)),
            locals: Mutex::new(HashMap::new()),
            out,
            tasks: Mutex::new(Vec::new()),
            dead: AtomicBool::new(false),
            ended: watch::channel(false).0,
            closing: Notify::new(),
            next_key: AtomicU64::new(1),
        });
        let first = lane.register();
        let writer = crate::node::output::spawn(write_loop(lane.clone(), send, queue));
        let reader = crate::node::output::spawn(read_loop(lane.clone(), recv));
        lane.track(writer.abort_handle());
        lane.track(reader.abort_handle());
        (lane, first)
    }

    /// A new local connection on this lane; `None` if the lane is dead.
    pub(super) fn attach(self: &Arc<Self>) -> Option<(BoxSend, BoxRecv)> {
        if self.is_dead() {
            return None;
        }
        Some(self.register())
    }

    fn register(self: &Arc<Self>) -> (BoxSend, BoxRecv) {
        let key = self.next_key.fetch_add(1, Ordering::Relaxed);
        let (client_end, mux_end) = tokio::io::duplex(LOCAL_PIPE);
        let (tx, replies) = mpsc::channel(LOCAL_QUEUE);
        lock(&self.locals).insert(key, tx);
        let task =
            crate::node::output::spawn(super::local::serve(self.clone(), key, mux_end, replies));
        self.track(task.abort_handle());
        let (read, write) = split(client_end);
        (Box::new(write), Box::new(read))
    }

    /// Remembers a task to abort on death; a task added to a dead lane is aborted at once.
    fn track(&self, handle: AbortHandle) {
        let mut tasks = lock(&self.tasks);
        if self.is_dead() {
            return handle.abort();
        }
        tasks.push(handle);
    }

    pub(super) fn locals(&self) -> usize {
        lock(&self.locals).len()
    }

    pub(super) fn is_dead(&self) -> bool {
        self.dead.load(Ordering::SeqCst)
    }

    /// Resolves when the lane has died.
    pub(super) async fn ended(&self) {
        let mut rx = self.ended.subscribe();
        let _ = rx.wait_for(|dead| *dead).await;
    }

    /// Ends the lane on purpose: the send side is finished, then the lane dies like after a break.
    pub(super) fn close(&self) {
        self.closing.notify_one();
    }

    /// The end of the lane, once: stops every task, which drops the local pipes so their readers see the end.
    fn die(&self) {
        let mut tasks = lock(&self.tasks);
        if self.dead.swap(true, Ordering::SeqCst) {
            return;
        }
        for handle in tasks.drain(..) {
            handle.abort();
        }
        lock(&self.locals).clear();
        self.ended.send_replace(true);
    }

    pub(super) async fn queue_out(&self, frame: RawFrame) -> bool {
        self.out.send(frame.into_bytes()).await.is_ok()
    }

    pub(super) fn reply_sender(&self, key: LocalKey) -> Option<mpsc::Sender<Vec<u8>>> {
        lock(&self.locals).get(&key).cloned()
    }

    pub(super) fn remove_local(&self, key: LocalKey) {
        lock(&self.locals).remove(&key);
    }

    /// The shared id for a local id, and the ids of idle bindings dropped to make room for a new one.
    pub(super) fn bind(&self, key: LocalKey, local_id: u16) -> (Option<u16>, Vec<u16>) {
        let mut table = lock(&self.table);
        let now = Instant::now();
        let swept = match table.has(key, local_id) {
            true => Vec::new(),
            false => table.sweep(now, SESSION_IDLE),
        };
        (table.bind(key, local_id, now), swept)
    }

    pub(super) fn unbind(&self, key: LocalKey, local_id: u16) -> Option<u16> {
        lock(&self.table).unbind(key, local_id)
    }

    pub(super) fn unbind_local(&self, key: LocalKey) -> Vec<u16> {
        lock(&self.table).unbind_local(key)
    }

    /// Hands a frame from the exit to its local connection with the local id restored.
    fn deliver(&self, frame: RawFrame) {
        if frame.kind() == Kind::Keepalive {
            return;
        }
        let owner = lock(&self.table).route_reply(frame.conn_id(), Instant::now());
        let Some((key, local_id)) = owner else { return };
        if frame.kind() == Kind::Close {
            lock(&self.table).unbind(key, local_id);
        }
        let Some(tx) = self.reply_sender(key) else {
            return;
        };
        let _ = tx.try_send(frame.with_conn_id(local_id).into_bytes());
    }
}

async fn write_loop(lane: Arc<Lane>, mut send: BoxSend, mut queue: mpsc::Receiver<Vec<u8>>) {
    loop {
        let bytes = tokio::select! {
            next = queue.recv() => next,
            () = lane.closing.notified() => {
                let _ = send.shutdown().await;
                None
            }
        };
        let Some(bytes) = bytes else { break };
        if send.write_all(&bytes).await.is_err() {
            break;
        }
    }
    lane.die();
}

async fn read_loop(lane: Arc<Lane>, mut recv: BoxRecv) {
    while let Ok(frame) = read_raw(&mut recv).await {
        lane.deliver(frame);
    }
    lane.die();
}

#[cfg(test)]
#[path = "lane_tests.rs"]
mod tests;
