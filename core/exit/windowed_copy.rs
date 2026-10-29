//! Copying with a cap on the bytes written but not yet flushed.
//!
//! The writer of an exit stream accepts data much faster than the peer acknowledges it, so a
//! fast download piles up in the transport's send queue. `copy_windowed` bounds that pile: after
//! `window` bytes it waits for `flush`, which for the exit stream settles when the peer has
//! acknowledged everything written so far.

use std::io;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite};

/// Copies `reader` to `writer` until EOF; returns the number of bytes copied.
///
/// `window` is the most bytes handed to `writer` between two completed flushes; 0 means no cap
/// and is exactly `tokio::io::copy`.
pub(crate) async fn copy_windowed<R, W>(
    reader: &mut R,
    writer: &mut W,
    window: usize,
) -> io::Result<u64>
where
    R: AsyncRead + Unpin + ?Sized,
    W: AsyncWrite + Unpin + ?Sized,
{
    match window {
        0 => tokio::io::copy(reader, writer).await,
        _ => tokio::io::copy(reader, &mut Windowed::new(writer, window)).await,
    }
}

/// A writer that flushes the inner one before more than `window` bytes pile up unflushed.
struct Windowed<'a, W: ?Sized> {
    inner: &'a mut W,
    window: usize,
    unflushed: usize,
}

impl<'a, W: ?Sized> Windowed<'a, W> {
    fn new(inner: &'a mut W, window: usize) -> Self {
        Self {
            inner,
            window,
            unflushed: 0,
        }
    }
}

impl<W: AsyncWrite + Unpin + ?Sized> AsyncWrite for Windowed<'_, W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = &mut *self;
        if this.unflushed >= this.window {
            ready!(Pin::new(&mut *this.inner).poll_flush(cx))?;
            this.unflushed = 0;
        }
        let room = this.window - this.unflushed;
        let take = buf.len().min(room);
        let written = ready!(Pin::new(&mut *this.inner).poll_write(cx, &buf[..take]))?;
        this.unflushed += written;
        Poll::Ready(Ok(written))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = &mut *self;
        ready!(Pin::new(&mut *this.inner).poll_flush(cx))?;
        this.unflushed = 0;
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut *self.inner).poll_shutdown(cx)
    }
}

#[cfg(test)]
#[path = "windowed_copy_stub.rs"]
pub(super) mod stub;

#[cfg(test)]
#[path = "windowed_copy_tests.rs"]
mod tests;
