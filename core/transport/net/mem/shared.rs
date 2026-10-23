//! Shared state of a `mem` connection: the stream table, frame parsing, closing.
//! The user handle and `PeerConn` are in `conn`; the stream halves are in `stream`.

use crate::transport::net::connect::{PacketPath, PathInfo};
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::mem::conn::CODE_PROTOCOL;
use crate::transport::net::mem::frame::Frame;
use crate::transport::net::mem::stream::{Stream, DATA_FRAME_HEADER_BYTES};
use crate::transport::net::mem::tasks;
use bytes::Bytes;
use portable_atomic::AtomicU64;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use tokio::sync::{mpsc, watch};

const MAX_LIVE_INCOMING_STREAMS: usize = 1024;

struct Table {
    streams: HashMap<u32, Arc<Stream>>,
    /// The next own id; `None` means exhausted.
    next_local: Option<u32>,
    max_peer_stream_id: u32,
    incoming_stream_count: usize,
}

/// Shared state of a connection: held by both tasks and the handles.
pub(crate) struct MemConnState {
    pub(super) path: Arc<dyn PacketPath>,
    is_dialer: bool,
    pub(super) out: mpsc::UnboundedSender<Frame>,
    table: Mutex<Table>,
    accept_tx: mpsc::UnboundedSender<u32>,
    close: OnceLock<(u32, Bytes)>,
    pub(super) closed_tx: watch::Sender<bool>,
    pub(super) sent_bytes_total: AtomicU64,
    pub(super) recv_bytes_total: AtomicU64,
}

/// The last dropped handle closes the connection with code 0.
pub(crate) struct ConnHandle {
    pub(super) shared: Arc<MemConnState>,
}

impl Drop for ConnHandle {
    fn drop(&mut self) {
        self.shared.close_local(0, Bytes::new());
    }
}

impl MemConnState {
    pub(super) fn path_info(&self) -> PathInfo {
        self.path.info()
    }

    pub(super) fn subscribe(&self) -> watch::Receiver<bool> {
        self.closed_tx.subscribe()
    }

    pub(super) fn sent_bytes(&self) -> u64 {
        self.sent_bytes_total.load(Ordering::Relaxed)
    }

    pub(super) fn recv_bytes(&self) -> u64 {
        self.recv_bytes_total.load(Ordering::Relaxed)
    }

    #[cfg(test)]
    pub(super) fn close_info(&self) -> Option<(u32, Bytes)> {
        self.close.get().cloned()
    }

    pub(super) fn is_closed(&self) -> bool {
        self.close.get().is_some()
    }

    pub(super) fn max_packet_bytes(&self) -> usize {
        self.path
            .max_packet_bytes()
            .max(DATA_FRAME_HEADER_BYTES + 1)
    }

    pub(super) fn send(&self, frame: Frame) {
        let _ = self.out.send(frame);
    }

    fn table(&self) -> MutexGuard<'_, Table> {
        self.table.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn is_mine(&self, id: u32) -> bool {
        (id & 1 == 1) == self.is_dialer
    }

    /// Removes a stream whose both `Fin`s have been delivered.
    pub(super) fn remove(&self, id: u32) {
        let mut t = self.table();
        if t.streams.remove(&id).is_some() && !self.is_mine(id) {
            t.incoming_stream_count -= 1;
        }
    }

    pub(super) fn lookup(&self, id: u32) -> Option<Arc<Stream>> {
        self.table().streams.get(&id).cloned()
    }

    /// A frame for a removed id is silently dropped; for a not yet opened one — a violation.
    fn gone_or_violation(&self, id: u32) -> Result<(), &'static str> {
        let t = self.table();
        let known = match self.is_mine(id) {
            true => t.next_local.is_none_or(|next| id < next),
            false => id <= t.max_peer_stream_id,
        };
        known
            .then_some(())
            .ok_or("frame for a stream that is not open")
    }

    fn mark_closed(&self, code: u32, reason: Bytes) -> bool {
        if self.close.set((code, reason)).is_err() {
            return false;
        }
        true
    }

    fn announce_closed(&self) {
        self.closed_tx.send_replace(true);
        let streams: Vec<_> = self.table().streams.values().cloned().collect();
        streams.iter().for_each(|s| s.wake_all());
    }

    pub(super) fn close_local(&self, code: u32, reason: Bytes) {
        let max = self.max_packet_bytes() - DATA_FRAME_HEADER_BYTES;
        let reason = reason.slice(..reason.len().min(max));
        if !self.mark_closed(code, reason.clone()) {
            return;
        }
        self.send(Frame::Close { code, reason });
        self.announce_closed();
    }

    pub(super) fn close_remote(&self, code: u32, reason: Bytes) {
        if self.mark_closed(code, reason) {
            self.announce_closed();
        }
    }

    pub(super) fn violation(&self, why: &'static str) {
        self.close_local(CODE_PROTOCOL, Bytes::from_static(why.as_bytes()));
    }

    pub(super) fn open_local(&self) -> NetResult<(u32, Arc<Stream>)> {
        let mut t = self.table();
        let id = t
            .next_local
            .ok_or_else(|| NetError::Other(anyhow::anyhow!("stream ids exhausted")))?;
        t.next_local = id.checked_add(2);
        let stream = Stream::new();
        t.streams.insert(id, stream.clone());
        self.send(Frame::Open(id));
        Ok((id, stream))
    }

    fn on_open(&self, id: u32) -> Result<(), &'static str> {
        let mut t = self.table();
        if self.is_mine(id) || id <= t.max_peer_stream_id {
            return Err("invalid stream id");
        }
        if t.incoming_stream_count >= MAX_LIVE_INCOMING_STREAMS {
            return Err("too many incoming streams");
        }
        t.max_peer_stream_id = id;
        t.incoming_stream_count += 1;
        t.streams.insert(id, Stream::new());
        let _ = self.accept_tx.send(id);
        Ok(())
    }

    fn with_stream(
        &self,
        id: u32,
        f: impl FnOnce(&Stream) -> Result<(), &'static str>,
    ) -> Result<(), &'static str> {
        match self.lookup(id) {
            Some(s) => f(&s),
            None => self.gone_or_violation(id),
        }
    }

    fn on_data(&self, id: u32, data: Bytes) -> Result<(), &'static str> {
        self.with_stream(id, |s| {
            s.on_data(id, data)
                .map(|reply| reply.into_iter().for_each(|f| self.send(f)))
        })
    }

    fn on_fin(&self, id: u32) -> Result<(), &'static str> {
        self.with_stream(id, |s| {
            if s.on_fin()? {
                self.remove(id);
            }
            Ok(())
        })
    }

    /// Parsing of a frame by the reader; `false` means there is no point reading further. It does not wait for the user.
    pub(super) fn dispatch(&self, frame: Frame) -> bool {
        let verdict = match frame {
            Frame::Open(id) => self.on_open(id),
            Frame::Data(id, data) => self.on_data(id, data),
            Frame::Fin(id) => self.on_fin(id),
            Frame::Window(id, inc) => self.with_stream(id, |s| s.on_window(inc)),
            Frame::Close { code, reason } => {
                self.close_remote(code, reason);
                return false;
            }
            Frame::Hello(_) | Frame::Accept | Frame::Refuse => {
                Err("handshake frame inside a connection")
            }
        };
        let Err(why) = verdict else {
            return true;
        };
        self.violation(why);
        false
    }
}

/// Starts the writer and the reader; `accept_rx` hands out the ids of streams opened by the counterpart.
pub(super) fn start(
    path: Arc<dyn PacketPath>,
    is_dialer: bool,
) -> (Arc<ConnHandle>, mpsc::UnboundedReceiver<u32>) {
    let (out, rx) = mpsc::unbounded_channel();
    let (accept_tx, accept_rx) = mpsc::unbounded_channel();
    let table = Table {
        streams: HashMap::new(),
        next_local: Some(if is_dialer { 1 } else { 2 }),
        max_peer_stream_id: 0,
        incoming_stream_count: 0,
    };
    let shared = Arc::new(MemConnState {
        path,
        is_dialer,
        out,
        table: Mutex::new(table),
        accept_tx,
        close: OnceLock::new(),
        closed_tx: watch::channel(false).0,
        sent_bytes_total: AtomicU64::new(0),
        recv_bytes_total: AtomicU64::new(0),
    });
    crate::transport::net::task_label::spawn(tasks::write_loop(shared.clone(), rx));
    crate::transport::net::task_label::spawn(tasks::read_loop(shared.clone()));
    (Arc::new(ConnHandle { shared }), accept_rx)
}
