//! A `mem` path: a pair of line halves with lossless queues, delay and a shared close flag.
//! The hub and the connector that hand out paths are in `hub` and `connector`.

use crate::net::connect::{PacketPath, PathInfo};
use crate::net::id::PeerId;
use crate::net::link::PathKind;
use crate::net::mem::hub::Hub;
use crate::net::stream::BoxFuture;
use bytes::Bytes;
use std::any::Any;
use std::io;
use std::sync::Arc;
use tokio::sync::{mpsc, watch, Mutex};
use tokio::time::Instant;

/// The largest packet of a `mem` path.
pub const MEM_MAX_PACKET: usize = 65_536;
/// Queue depth of one direction; when the queue is full, `send` waits.
const QUEUE: usize = 1024;

type TimedPacket = (Instant, Bytes);

/// The part shared by the two halves: the close flags of the sides and who is at the ends.
pub(super) struct Line {
    peers: [PeerId; 2],
    slots: [u64; 2],
    closed: [watch::Sender<bool>; 2],
}

impl Line {
    fn new(peers: [PeerId; 2], slots: [u64; 2]) -> Arc<Line> {
        let closed = [watch::channel(false).0, watch::channel(false).0];
        Arc::new(Line {
            peers,
            slots,
            closed,
        })
    }

    pub(super) fn close_side(&self, side: usize) {
        self.closed[side].send_replace(true);
    }

    pub(super) fn close_all(&self) {
        self.close_side(0);
        self.close_side(1);
    }

    /// Closes the sides that belong to the slot.
    pub(super) fn close_slot(&self, slot: u64) {
        for side in 0..2 {
            self.close_if_slot(side, slot);
        }
    }

    fn close_if_slot(&self, side: usize, slot: u64) {
        if self.slots[side] == slot {
            self.close_side(side);
        }
    }

    /// Whether the line connects exactly this pair of nodes (in either order).
    pub(super) fn joins(&self, a: PeerId, b: PeerId) -> bool {
        (self.peers[0] == a && self.peers[1] == b) || (self.peers[0] == b && self.peers[1] == a)
    }

    fn is_closed(&self, side: usize) -> bool {
        *self.closed[side].borrow()
    }

    async fn wait(&self, side: usize) {
        let mut rx = self.closed[side].subscribe();
        let _ = rx.wait_for(|v| *v).await;
    }

    async fn wait_either(&self) {
        tokio::select! {
            _ = self.wait(0) => {}
            _ = self.wait(1) => {}
        }
    }
}

struct Inbox {
    rx: mpsc::Receiver<TimedPacket>,
    /// An accepted but not yet delivered packet (waits for its timestamp); kept so that `recv` can be cancelled.
    pending: Option<TimedPacket>,
}

/// A half of a line: the path of one node to another.
pub struct MemPath {
    hub: Arc<Hub>,
    line: Arc<Line>,
    side: usize,
    info: PathInfo,
    tx: mpsc::Sender<TimedPacket>,
    inbox: Mutex<Inbox>,
}

fn eof() -> io::Error {
    io::Error::from(io::ErrorKind::UnexpectedEof)
}

fn broken() -> io::Error {
    io::Error::from(io::ErrorKind::BrokenPipe)
}

impl MemPath {
    /// A pair of halves of one line; `a` and `b` are (node, slot). The hub is not blocked.
    pub(super) fn pair(hub: &Arc<Hub>, a: (PeerId, u64), b: (PeerId, u64)) -> (MemPath, MemPath) {
        let line = Line::new([a.0, b.0], [a.1, b.1]);
        let (ab_tx, ab_rx) = mpsc::channel(QUEUE);
        let (ba_tx, ba_rx) = mpsc::channel(QUEUE);
        let first = Self::half(hub, &line, 0, b.0, ab_tx, ba_rx);
        let second = Self::half(hub, &line, 1, a.0, ba_tx, ab_rx);
        (first, second)
    }

    fn half(
        hub: &Arc<Hub>,
        line: &Arc<Line>,
        side: usize,
        remote: PeerId,
        tx: mpsc::Sender<TimedPacket>,
        rx: mpsc::Receiver<TimedPacket>,
    ) -> MemPath {
        let info = PathInfo {
            peer: remote,
            kind: PathKind::Direct,
            remote: None,
            family: "mem",
        };
        MemPath {
            hub: hub.clone(),
            line: line.clone(),
            side,
            info,
            tx,
            inbox: Mutex::new(Inbox { rx, pending: None }),
        }
    }

    pub(super) fn line(&self) -> &Arc<Line> {
        &self.line
    }

    fn either_closed(&self) -> bool {
        self.line.is_closed(0) || self.line.is_closed(1)
    }

    async fn send_inner(&self, pkt: Bytes) -> io::Result<()> {
        if pkt.len() > MEM_MAX_PACKET {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        }
        if self.either_closed() {
            return Err(broken());
        }
        let permit = tokio::select! {
            p = self.tx.reserve() => p.map_err(|_| broken())?,
            _ = self.line.wait_either() => return Err(broken()),
        };
        permit.send((Instant::now() + self.hub.delay(), pkt));
        Ok(())
    }

    /// The next packet of the queue; closing of its own side is an immediate end, of the other side — after the remainder.
    async fn next_item(&self, inbox: &mut Inbox) -> io::Result<TimedPacket> {
        loop {
            if self.line.is_closed(self.side) {
                return Err(eof());
            }
            if let Some(item) = inbox.pending.take() {
                return Ok(item);
            }
            if let Some(item) = self.poll_queue(inbox).await? {
                return Ok(item);
            }
        }
    }

    /// One waiting step: a packet, `None` (recheck the flags) or the end.
    async fn poll_queue(&self, inbox: &mut Inbox) -> io::Result<Option<TimedPacket>> {
        match inbox.rx.try_recv() {
            Ok(item) => return Ok(Some(item)),
            Err(mpsc::error::TryRecvError::Disconnected) => return Err(eof()),
            Err(mpsc::error::TryRecvError::Empty) => {}
        }
        if self.line.is_closed(1 - self.side) {
            return Err(eof());
        }
        tokio::select! {
            r = inbox.rx.recv() => r.map(Some).ok_or_else(eof),
            _ = self.line.wait_either() => Ok(None),
        }
    }

    async fn recv_inner(&self) -> io::Result<Bytes> {
        let mut inbox = self.inbox.lock().await;
        let item = self.next_item(&mut inbox).await?;
        let mark = item.0;
        inbox.pending = Some(item);
        tokio::select! {
            _ = tokio::time::sleep_until(mark) => {}
            _ = self.line.wait(self.side) => return Err(eof()),
        }
        Ok(inbox.pending.take().map(|i| i.1).unwrap_or_default())
    }
}

impl PacketPath for MemPath {
    fn info(&self) -> PathInfo {
        self.info.clone()
    }

    fn max_packet_bytes(&self) -> usize {
        MEM_MAX_PACKET
    }

    fn send(&self, pkt: Bytes) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(self.send_inner(pkt))
    }

    fn recv(&self) -> BoxFuture<'_, io::Result<Bytes>> {
        Box::pin(self.recv_inner())
    }

    fn close(&self) {
        self.line.close_side(self.side);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Drop for MemPath {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
#[path = "path_tests.rs"]
mod tests;
