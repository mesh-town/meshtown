//! Proxy tasks: receiving with a decision on the packet's fate, deferred delivery and a silent sink.
//!
//! What it does: `ingest` reads the socket, asks `Direction` what to do with the packet and puts
//! the ones to deliver into a time queue; `deliver` waits for the appointed moment and sends. Order
//! is preserved: the exit moment does not decrease, the queue is FIFO.
//! `sink` eats everything that arrived at an occupied foreign address. What it does not have: the loss and queue rules themselves (`model.rs`) and the proxy assembly (`mod.rs`).

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use tokio::net::UdpSocket;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::time::sleep_until;

use super::model::{Direction, Verdict};
use super::stats::Counters;

/// A direction's peer: whose address we accept and where we reply.
pub trait Peer: Send + Sync {
    /// Whether the datagram's sender is acceptable; the client is remembered by the first one.
    fn admits(&self, src: SocketAddr) -> bool;
    /// The address to send to, if already known.
    fn addr(&self) -> Option<SocketAddr>;
}

/// The client: the address is unknown until the first datagram arrives.
#[derive(Default)]
pub struct Client(pub OnceLock<SocketAddr>);

impl Peer for Client {
    fn admits(&self, src: SocketAddr) -> bool {
        *self.0.get_or_init(|| src) == src
    }

    fn addr(&self) -> Option<SocketAddr> {
        self.0.get().copied()
    }
}

/// The target: the address is known in advance.
pub struct Target(pub SocketAddr);

impl Peer for Target {
    fn admits(&self, src: SocketAddr) -> bool {
        self.0 == src
    }

    fn addr(&self) -> Option<SocketAddr> {
        Some(self.0)
    }
}

/// The kill switch takes only packets not shorter than this length: ACK-only and PING are shorter.
const TRAP_MIN_LEN: usize = 1000;

/// Atomically decrements a nonzero counter by 1; `true` if it decremented.
#[allow(deprecated)] // `try_update` needs Rust 1.95, above the MSRV (1.94)
fn take_trap(trap: &AtomicUsize) -> bool {
    trap.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| v.checked_sub(1))
        .is_ok()
}

/// A direction's levers that change on the fly: the kill switch and the random loss share (thousandths of a percent).
#[derive(Default, Clone)]
pub struct Knobs {
    pub trap: Arc<AtomicUsize>,
    pub loss_milli: Arc<AtomicU32>,
}

pub type Scheduled = (Instant, Vec<u8>);

/// Receiving: reads `sock`, applies the model and puts what is to be delivered into `queue`.
pub async fn ingest(
    sock: Arc<UdpSocket>,
    from: Arc<dyn Peer>,
    mut dir: Direction,
    counters: Arc<Counters>,
    queue: UnboundedSender<Scheduled>,
    knobs: Knobs,
) {
    let mut buf = vec![0u8; 65536];
    while let Ok((n, src)) = sock.recv_from(&mut buf).await {
        if !from.admits(src) {
            continue;
        }
        counters.received();
        let data = n >= TRAP_MIN_LEN;
        if data
            && (take_trap(&knobs.trap) || dir.extra_lost(knobs.loss_milli.load(Ordering::SeqCst)))
        {
            counters.lost_model();
            continue;
        }
        let verdict = dir.admit(Instant::now(), n);
        if !schedule(verdict, &buf[..n], &counters, &queue) {
            return;
        }
    }
}

/// Records a packet's fate; `false` — the queue's receiver is gone, it is time to end the task.
fn schedule(
    verdict: Verdict,
    data: &[u8],
    counters: &Counters,
    queue: &UnboundedSender<Scheduled>,
) -> bool {
    match verdict {
        Verdict::Lost => counters.lost_model(),
        Verdict::Dropped => counters.dropped_queue(),
        Verdict::Deliver(at) => return queue.send((at, data.to_vec())).is_ok(),
    }
    true
}

/// Delivery: waits for the moment, sends through `sock` to the addressee `to`.
pub async fn deliver(
    sock: Arc<UdpSocket>,
    to: Arc<dyn Peer>,
    counters: Arc<Counters>,
    mut queue: UnboundedReceiver<Scheduled>,
) {
    while let Some((at, data)) = queue.recv().await {
        sleep_until(tokio::time::Instant::from_std(at)).await;
        let Some(addr) = to.addr() else { continue };
        counters.delivered();
        let _ = sock.send_to(&data, addr).await;
    }
}

/// The silent sink: reads `sock` and discards everything, counting what arrived.
pub async fn sink(sock: UdpSocket, counters: Arc<Counters>) {
    let mut buf = vec![0u8; 65536];
    while sock.recv_from(&mut buf).await.is_ok() {
        counters.received();
    }
}
