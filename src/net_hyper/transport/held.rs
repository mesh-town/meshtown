//! A stream that holds a slot in the dial queue until the other side's first reply arrives.
//! The townjump server puts ready connections into a 16-slot queue and drops the extra ones
//! on overflow; while the dialing side waits for `Ack`, its slot is taken, and a burst does not exceed the queue.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::OwnedSemaphorePermit;

pub struct Held<S> {
    inner: S,
    slot: Option<OwnedSemaphorePermit>,
}

impl<S> Held<S> {
    pub fn new(inner: S, slot: OwnedSemaphorePermit) -> Self {
        Self { inner, slot: Some(slot) }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for Held<S> {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        let res = Pin::new(&mut self.inner).poll_read(cx, buf);
        if res.is_ready() {
            self.slot = None;
        }
        res
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for Held<S> {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, data: &[u8]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, data)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

#[cfg(test)]
#[path = "held_tests.rs"]
mod held_tests;
