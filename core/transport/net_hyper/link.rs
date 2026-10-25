//! Session counters and path and network information for display. For now the path is always "direct UDX",
//! the window is unknown; rtt comes from the connection probe.

use crate::transport::net::link::{LinkInfo, NetInfo, PathKind};
use portable_atomic::AtomicU64;
use std::io;
use std::net::{SocketAddr, SocketAddrV4};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use townjump_dht::{DhtConnection, RttProbe};

/// What is known about the path of a session once it is set up.
#[derive(Clone, Default)]
pub struct PathFacts {
    pub remote: Option<SocketAddr>,
    pub rtt: Option<RttProbe>,
}

impl PathFacts {
    /// Outgoing session: `via` is the address a direct dial used (`None` for a dial by key).
    pub fn dialed(conn: &DhtConnection, via: Option<SocketAddrV4>) -> PathFacts {
        PathFacts {
            remote: via.map(SocketAddr::V4),
            rtt: conn.rtt_probe(),
        }
    }

    /// Incoming session: the source address of a handshake is not a dialable address, so it is not kept.
    pub fn accepted(conn: &DhtConnection) -> PathFacts {
        PathFacts {
            remote: None,
            rtt: conn.rtt_probe(),
        }
    }
}

/// Counters of one session; shared by all its streams.
#[derive(Default)]
pub struct LinkMeter {
    sent: AtomicU64,
    recv: AtomicU64,
    frames: AtomicU64,
    established: AtomicBool,
    closed: AtomicBool,
    path: OnceLock<PathFacts>,
}

impl LinkMeter {
    pub fn new() -> Self {
        Self::default()
    }

    /// The control stream received `Ack`: the path is established.
    pub fn mark_established(&self) {
        self.established.store(true, Relaxed);
    }

    /// Records the path facts; the first call wins.
    pub fn note_path(&self, facts: PathFacts) {
        let _ = self.path.set(facts);
    }

    /// The session is closed: data streams answer `ConnectionAborted` from now on.
    pub fn close(&self) {
        self.closed.store(true, Relaxed);
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Relaxed)
    }

    fn alive(&self) -> bool {
        self.established.load(Relaxed) && !self.is_closed()
    }
}

/// Path information of the session: bytes are mesh, not UDP.
pub fn link_info(meter: &LinkMeter) -> LinkInfo {
    LinkInfo {
        path: path_of(meter),
        sent_bytes: meter.sent.load(Relaxed),
        recv_bytes: meter.recv.load(Relaxed),
        path_count: usize::from(meter.alive()),
        stream_frames: meter.frames.load(Relaxed),
        remote: meter.path.get().and_then(|p| p.remote),
        rtt: probed_rtt(meter),
    }
}

fn probed_rtt(meter: &LinkMeter) -> Option<std::time::Duration> {
    let probe = meter.path.get()?.rtt.as_ref()?;
    probe()
}

fn path_of(meter: &LinkMeter) -> PathKind {
    if meter.established.load(Relaxed) {
        return PathKind::Direct;
    }
    PathKind::Unknown
}

/// Network information of the node: `seen` is the `listen` outcome (`None` — not known yet).
pub fn net_info(seen: Option<bool>) -> NetInfo {
    NetInfo { net_seen: seen }
}

/// A stream with byte counting; after the session closes it cannot be read or written.
pub struct Metered<S> {
    inner: S,
    meter: Arc<LinkMeter>,
}

impl<S> Metered<S> {
    pub fn new(inner: S, meter: Arc<LinkMeter>) -> Self {
        Self { inner, meter }
    }
}

fn aborted() -> io::Error {
    io::Error::new(io::ErrorKind::ConnectionAborted, "session closed")
}

impl<S: AsyncRead + Unpin> AsyncRead for Metered<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.meter.is_closed() {
            return Poll::Ready(Err(aborted()));
        }
        let before = buf.filled().len();
        let res = Pin::new(&mut self.inner).poll_read(cx, buf);
        self.meter
            .recv
            .fetch_add((buf.filled().len() - before) as u64, Relaxed);
        res
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for Metered<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        if self.meter.is_closed() {
            return Poll::Ready(Err(aborted()));
        }
        let res = Pin::new(&mut self.inner).poll_write(cx, data);
        if let Poll::Ready(Ok(n)) = &res {
            self.meter.sent.fetch_add(*n as u64, Relaxed);
            self.meter.frames.fetch_add(1, Relaxed);
        }
        res
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

#[cfg(test)]
#[path = "link_tests.rs"]
mod tests;
