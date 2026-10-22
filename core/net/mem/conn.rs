//! The `mem` connection: `MemConn` as a `PeerConn` on top of shared state.
//! The stream table, closing and tasks are in `shared`; the stream halves are in `stream`.

use crate::net::connect::PacketPath;
use crate::net::error::{NetError, NetResult};
use crate::net::id::PeerId;
use crate::net::link::LinkInfo;
use crate::net::mem::shared::{start, ConnHandle, MemConnState};
use crate::net::mem::stream::{halves, Stream};
use crate::net::mem::tasks::wait_until_closed;
use crate::net::stream::{BoxFuture, StreamPair};
use crate::net::transport::PeerConn;
use bytes::Bytes;
use std::sync::atomic::Ordering;
use portable_atomic::AtomicU64;
use std::sync::Arc;
use tokio::sync::mpsc;

/// In each direction.
pub(crate) const STREAM_WINDOW_BYTES: usize = 256 * 1024;
/// `Close` code when the counterpart violates the protocol.
pub(crate) const CODE_PROTOCOL: u32 = 0xFFFF_0001;
/// Close code when the path broke off without a `Close`.
pub(crate) const CODE_PATH_LOST: u32 = 0xFFFF_0002;

static NEXT_STABLE: AtomicU64 = AtomicU64::new(1);

pub(crate) struct MemConn {
    handle: Arc<ConnHandle>,
    accept_rx: tokio::sync::Mutex<mpsc::UnboundedReceiver<u32>>,
    stable: u64,
}

impl MemConn {
    pub(crate) fn spawn(path: Arc<dyn PacketPath>, is_dialer: bool) -> Arc<MemConn> {
        let (handle, accept_rx) = start(path, is_dialer);
        Arc::new(MemConn {
            handle,
            accept_rx: tokio::sync::Mutex::new(accept_rx),
            stable: NEXT_STABLE.fetch_add(1, Ordering::Relaxed),
        })
    }

    fn shared(&self) -> &Arc<MemConnState> {
        &self.handle.shared
    }

    fn pair(&self, id: u32, stream: Arc<Stream>) -> StreamPair {
        let (send, recv) = halves(id, stream, self.handle.clone());
        StreamPair {
            send: Box::new(send),
            recv: Box::new(recv),
        }
    }

    async fn open_inner(&self) -> NetResult<StreamPair> {
        if self.shared().is_closed() {
            return Err(NetError::Closed);
        }
        let (id, stream) = self.shared().open_local()?;
        Ok(self.pair(id, stream))
    }

    async fn accept_inner(&self) -> NetResult<StreamPair> {
        let mut closed = self.shared().subscribe();
        let mut rx = self.accept_rx.lock().await;
        let id = tokio::select! {
            biased;
            _ = wait_until_closed(&mut closed) => return Err(NetError::Closed),
            id = rx.recv() => id.ok_or(NetError::Closed)?,
        };
        let stream = self.shared().lookup(id).ok_or(NetError::Closed)?;
        Ok(self.pair(id, stream))
    }

    #[cfg(test)]
    pub(crate) fn close_info(&self) -> Option<(u32, Bytes)> {
        self.shared().close_info()
    }
}

impl PeerConn for MemConn {
    fn peer(&self) -> PeerId {
        self.shared().path_info().peer
    }

    fn stable_id(&self) -> u64 {
        self.stable
    }

    fn open(&self) -> BoxFuture<'_, NetResult<StreamPair>> {
        Box::pin(self.open_inner())
    }

    fn accept(&self) -> BoxFuture<'_, NetResult<StreamPair>> {
        Box::pin(self.accept_inner())
    }

    fn link(&self) -> LinkInfo {
        let info = self.shared().path_info();
        LinkInfo {
            path: info.kind,
            remote: info.remote,
            sent_bytes: self.shared().sent_bytes(),
            recv_bytes: self.shared().recv_bytes(),
            ..LinkInfo::default()
        }
    }

    fn close(&self, code: u32, reason: &[u8]) {
        self.shared()
            .close_local(code, Bytes::copy_from_slice(reason));
    }

    fn closed(&self) -> BoxFuture<'_, ()> {
        let mut rx = self.shared().subscribe();
        Box::pin(async move { wait_until_closed(&mut rx).await })
    }

    fn is_closed(&self) -> bool {
        self.shared().is_closed()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
#[path = "conn_close_tests.rs"]
mod close_tests;
#[cfg(test)]
#[path = "conn_tests.rs"]
mod tests;
