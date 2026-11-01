//! Progress of one splice direction, so that the wait for the second half can tell an idle
//! half from a busy one: `Counted` counts the bytes a writer accepted, `wait_second_half` waits on idleness.

use std::io;
use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::task::{Context, Poll};

use portable_atomic::AtomicU64;
use tokio::io::AsyncWrite;
use tokio::task::JoinHandle;
use tokio::time::timeout;

use super::splice::half_close_grace_after;

/// A shared count of the bytes one direction has moved.
#[derive(Clone, Default)]
pub(super) struct Progress(Arc<AtomicU64>);

impl Progress {
    pub(super) fn add(&self, n: u64) {
        self.0.fetch_add(n, Ordering::Relaxed);
    }

    pub(super) fn seen(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

/// An `AsyncWrite` that adds every accepted byte to a `Progress`; flush and shutdown pass through.
pub(super) struct Counted<W> {
    inner: W,
    progress: Progress,
}

impl<W> Counted<W> {
    pub(super) fn new(inner: W, progress: Progress) -> Self {
        Self { inner, progress }
    }
}

impl<W: AsyncWrite + Unpin> AsyncWrite for Counted<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let polled = Pin::new(&mut self.inner).poll_write(cx, buf);
        if let Poll::Ready(Ok(n)) = &polled {
            self.progress.add(*n as u64);
        }
        polled
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// Waits for the second half after the first one closed, counting the wait from idleness: each time the
/// grace period expires, the wait is renewed if the half moved bytes meanwhile, and ends (with 0) if it did not.
/// So a half idle from the start is cut after one to two grace periods, and a busy one is never cut.
pub(super) async fn wait_second_half(
    task: &mut JoinHandle<u64>,
    progress: &Progress,
    first_bytes: u64,
) -> u64 {
    let mut before = progress.seen();
    loop {
        match timeout(half_close_grace_after(first_bytes), &mut *task).await {
            Ok(joined) => return joined.unwrap_or(0),
            Err(_) if progress.seen() == before => return 0,
            Err(_) => before = progress.seen(),
        }
    }
}

#[cfg(test)]
#[path = "splice_progress_tests.rs"]
mod tests;
