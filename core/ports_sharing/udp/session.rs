//! A stream pair as one `AsyncRead + AsyncWrite` with an idle limit and an optional slot.
//!
//! What it does: after `idle` without a read or a write both directions fail with a timeout error, and the end of
//! the remote side is reported as an error, not as EOF (the framing loop stops polling reads after EOF and would keep
//! the stream and the slot). The slot is released when the session is dropped.
//! What it does not do: it does not frame datagrams and does not open streams.

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{ready, Context, Poll};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::OwnedSemaphorePermit;
use tokio::time::{sleep, Instant, Sleep};

use crate::transport::net::{BoxRecv, BoxSend};

/// Idle limit of a session on the opening side.
pub const OPEN_IDLE: Duration = Duration::from_secs(30);
/// Idle limit of a session on the publishing side (a backstop: a dropped stream usually ends it earlier).
pub const SERVE_IDLE: Duration = Duration::from_secs(45);
/// Simultaneous local senders served by one opened UDP port.
pub const MAX_SENDERS_PER_OPEN: usize = 128;
/// Simultaneous UDP sessions served by the publishing side.
pub const MAX_UDP_SESSIONS_SERVED: usize = 256;
/// Prefix of our own session errors; the crate log bridge drops lines carrying it.
pub const UDP_SESSION_MARK: &str = "udp session: ";

pub struct UdpSession {
    send: BoxSend,
    recv: BoxRecv,
    idle: Duration,
    // One timer per direction: a timer keeps only the last waker, and the two halves may be polled by different tasks.
    read_timer: Pin<Box<Sleep>>,
    write_timer: Pin<Box<Sleep>>,
    idled: bool,
    _permit: Option<OwnedSemaphorePermit>,
}

impl UdpSession {
    pub fn new(
        send: BoxSend,
        recv: BoxRecv,
        idle: Duration,
        permit: Option<OwnedSemaphorePermit>,
    ) -> Self {
        UdpSession {
            send,
            recv,
            idle,
            read_timer: Box::pin(sleep(idle)),
            write_timer: Box::pin(sleep(idle)),
            idled: false,
            _permit: permit,
        }
    }

    fn touch(&mut self) {
        let deadline = Instant::now() + self.idle;
        self.read_timer.as_mut().reset(deadline);
        self.write_timer.as_mut().reset(deadline);
    }
}

fn session_error(kind: io::ErrorKind, what: &str) -> io::Error {
    io::Error::new(kind, format!("{UDP_SESSION_MARK}{what}"))
}

fn idle_error() -> io::Error {
    session_error(io::ErrorKind::TimedOut, "idle")
}

/// Pending while the timer runs; when it fires, the session is marked idle and the error is ready.
fn poll_idle(timer: Pin<&mut Sleep>, idled: &mut bool, cx: &mut Context<'_>) -> Poll<io::Error> {
    ready!(timer.poll(cx));
    *idled = true;
    Poll::Ready(idle_error())
}

impl AsyncRead for UdpSession {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = &mut *self;
        if this.idled {
            return Poll::Ready(Err(idle_error()));
        }
        if buf.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        let before = buf.filled().len();
        match Pin::new(&mut this.recv).poll_read(cx, buf) {
            Poll::Ready(Ok(())) if buf.filled().len() == before => {
                Poll::Ready(Err(session_error(io::ErrorKind::UnexpectedEof, "closed")))
            }
            Poll::Ready(Ok(())) => {
                this.touch();
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => poll_idle(this.read_timer.as_mut(), &mut this.idled, cx).map(Err),
        }
    }
}

impl AsyncWrite for UdpSession {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = &mut *self;
        if this.idled {
            return Poll::Ready(Err(idle_error()));
        }
        match Pin::new(&mut this.send).poll_write(cx, data) {
            Poll::Ready(Ok(n)) => {
                this.touch();
                Poll::Ready(Ok(n))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => poll_idle(this.write_timer.as_mut(), &mut this.idled, cx).map(Err),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.send).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.send).poll_shutdown(cx)
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod session_tests;
