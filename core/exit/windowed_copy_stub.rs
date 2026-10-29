//! A test writer that behaves like a transport whose `flush` waits for the peer's acknowledgement.
//!
//! `AckWriter` accepts every byte it is given (or at most `cap_writes` per write) and counts the bytes not yet acknowledged. In `Manual` mode
//! `poll_flush` stays pending until the test calls `AckHandle::ack_all`; in `Auto` mode every flush settles
//! at once. `tokio::io::duplex` cannot stand in for it: its flush never waits.

use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context, Poll, Waker};
use std::time::Duration;
use tokio::io::AsyncWrite;

/// How a flush settles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AckMode {
    /// Every flush completes at once, acknowledging everything written so far.
    Auto,
    /// A flush with unacknowledged bytes waits until `AckHandle::ack_all`.
    Manual,
}

#[derive(Default)]
struct State {
    written: Vec<u8>,
    unacked: usize,
    max_unacked: usize,
    flushes: usize,
    flush_error: Option<io::ErrorKind>,
    write_error: Option<io::ErrorKind>,
    write_cap: Option<usize>,
    flush_waker: Option<Waker>,
}

/// The writing half given to the code under test; also fits a `BoxSend`.
pub(crate) struct AckWriter {
    mode: AckMode,
    state: Arc<Mutex<State>>,
}

/// The test's view of an `AckWriter`: what it received and the controls over acknowledgements and failures.
#[derive(Clone)]
pub(crate) struct AckHandle {
    state: Arc<Mutex<State>>,
}

/// A writer and the handle to it.
pub(crate) fn ack_writer(mode: AckMode) -> (AckWriter, AckHandle) {
    let state = Arc::new(Mutex::new(State::default()));
    let writer = AckWriter {
        mode,
        state: state.clone(),
    };
    (writer, AckHandle { state })
}

fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(|e| e.into_inner())
}

impl AsyncWrite for AckWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let mut st = lock(&self.state);
        if let Some(kind) = st.write_error {
            return Poll::Ready(Err(io::Error::new(kind, "injected write failure")));
        }
        let take = st.write_cap.map_or(buf.len(), |cap| cap.min(buf.len()));
        st.written.extend_from_slice(&buf[..take]);
        st.unacked += take;
        st.max_unacked = st.max_unacked.max(st.unacked);
        Poll::Ready(Ok(take))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let mut st = lock(&self.state);
        st.flushes += 1;
        if let Some(kind) = st.flush_error {
            return Poll::Ready(Err(io::Error::new(kind, "injected flush failure")));
        }
        if self.mode == AckMode::Auto || st.unacked == 0 {
            st.unacked = 0;
            return Poll::Ready(Ok(()));
        }
        st.flush_waker = Some(cx.waker().clone());
        Poll::Pending
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

impl AckHandle {
    /// Acknowledges everything written so far and wakes a pending flush.
    pub(crate) fn ack_all(&self) {
        let mut st = lock(&self.state);
        st.unacked = 0;
        if let Some(waker) = st.flush_waker.take() {
            waker.wake();
        }
    }

    /// Every following flush fails with `kind`.
    pub(crate) fn fail_flush(&self, kind: io::ErrorKind) {
        lock(&self.state).flush_error = Some(kind);
    }

    /// Every following write fails with `kind`.
    pub(crate) fn fail_write(&self, kind: io::ErrorKind) {
        lock(&self.state).write_error = Some(kind);
    }

    /// Every following write takes at most `n` bytes, like a transport with little room left.
    pub(crate) fn cap_writes(&self, n: usize) {
        lock(&self.state).write_cap = Some(n);
    }

    /// Everything accepted so far.
    pub(crate) fn written(&self) -> Vec<u8> {
        lock(&self.state).written.clone()
    }

    pub(crate) fn written_len(&self) -> usize {
        lock(&self.state).written.len()
    }

    /// The most bytes that were ever accepted and not yet acknowledged at the same time.
    pub(crate) fn max_unacked(&self) -> usize {
        lock(&self.state).max_unacked
    }

    /// How many times `poll_flush` was called.
    pub(crate) fn flushes(&self) -> usize {
        lock(&self.state).flushes
    }

    /// Waits until exactly `n` bytes are accepted; panics on more than `n` or when `limit` runs out.
    pub(crate) async fn wait_written(&self, n: usize, limit: Duration) {
        let reached = tokio::time::timeout(limit, async {
            while self.written_len() < n {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await;
        assert!(
            reached.is_ok(),
            "only {} of {n} bytes arrived",
            self.written_len()
        );
        assert_eq!(self.written_len(), n, "more bytes arrived than expected");
    }

    /// Waits until `poll_flush` was called at least `n` times; panics when `limit` runs out.
    pub(crate) async fn wait_flushes(&self, n: usize, limit: Duration) {
        let reached = tokio::time::timeout(limit, async {
            while self.flushes() < n {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await;
        assert!(
            reached.is_ok(),
            "only {} of {n} flushes happened",
            self.flushes()
        );
    }
}

/// `len` bytes that don't repeat within a window, so a lost or shifted byte shows.
pub(crate) fn pattern(len: usize) -> Vec<u8> {
    (0..len)
        .map(|i| u8::try_from(i % 251).unwrap_or(0))
        .collect()
}
