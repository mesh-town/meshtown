//! Halves of a `mem` stream: receive buffer, write credit, `Fin` and window return.
//! The connection, the stream table and closing are in `conn`.

use crate::transport::net::mem::conn::STREAM_WINDOW_BYTES;
use crate::transport::net::mem::frame::Frame;
use crate::transport::net::mem::shared::ConnHandle;
use bytes::Bytes;
use std::collections::VecDeque;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context, Poll, Waker};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub(super) const DATA_FRAME_HEADER_BYTES: usize = 5;

pub(super) struct StreamState {
    buf: VecDeque<Bytes>,
    buffered_bytes: usize,
    /// Read but not yet returned to the counterpart as a window.
    window_owed_bytes: usize,
    fin_in: bool,
    recv_dropped: bool,
    read_waker: Option<Waker>,
    write_credit_bytes: usize,
    fin_out: bool,
    write_waker: Option<Waker>,
}

pub(super) struct Stream {
    inner: Mutex<StreamState>,
}

fn wake(w: &mut Option<Waker>) {
    if let Some(w) = w.take() {
        w.wake();
    }
}

impl StreamState {
    fn take(&mut self, out: &mut ReadBuf<'_>) {
        while out.remaining() > 0 {
            let Some(front) = self.buf.front_mut() else {
                return;
            };
            let n = front.len().min(out.remaining());
            out.put_slice(&front.split_to(n));
            self.buffered_bytes -= n;
            self.window_owed_bytes += n;
            if front.is_empty() {
                self.buf.pop_front();
            }
        }
    }

    /// Window increment, if at least half has been read.
    fn window_due(&mut self) -> Option<u32> {
        if self.window_owed_bytes < STREAM_WINDOW_BYTES / 2 {
            return None;
        }
        let n = std::mem::take(&mut self.window_owed_bytes);
        Some(n as u32)
    }

    fn done(&self) -> bool {
        self.fin_in && self.fin_out
    }
}

impl Stream {
    pub(super) fn new() -> Arc<Stream> {
        Arc::new(Stream {
            inner: Mutex::new(StreamState {
                buf: VecDeque::new(),
                buffered_bytes: 0,
                window_owed_bytes: 0,
                fin_in: false,
                recv_dropped: false,
                read_waker: None,
                write_credit_bytes: STREAM_WINDOW_BYTES,
                fin_out: false,
                write_waker: None,
            }),
        })
    }

    fn lock(&self) -> MutexGuard<'_, StreamState> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// `Data` from the counterpart: `Ok(Some(frame))` — reply with a window; `Err` — a violation.
    pub(super) fn on_data(&self, id: u32, data: Bytes) -> Result<Option<Frame>, &'static str> {
        let mut g = self.lock();
        if g.recv_dropped {
            return Ok(Some(Frame::Window(id, data.len() as u32)));
        }
        if g.fin_in {
            return Err("data after Fin");
        }
        if g.buffered_bytes + data.len() > STREAM_WINDOW_BYTES {
            return Err("data beyond the window");
        }
        g.buffered_bytes += data.len();
        g.buf.push_back(data);
        wake(&mut g.read_waker);
        Ok(None)
    }

    /// `Fin` from the counterpart: `Ok(true)` — both `Fin`s are delivered, the stream can be removed.
    pub(super) fn on_fin(&self) -> Result<bool, &'static str> {
        let mut g = self.lock();
        if g.fin_in {
            return Err("repeated Fin");
        }
        g.fin_in = true;
        wake(&mut g.read_waker);
        Ok(g.done())
    }

    pub(super) fn on_window(&self, inc: u32) -> Result<(), &'static str> {
        let mut g = self.lock();
        if g.write_credit_bytes + inc as usize > STREAM_WINDOW_BYTES {
            return Err("window larger than the limit");
        }
        g.write_credit_bytes += inc as usize;
        wake(&mut g.write_waker);
        Ok(())
    }

    /// Connection closed: wake both halves.
    pub(super) fn wake_all(&self) {
        let mut g = self.lock();
        wake(&mut g.read_waker);
        wake(&mut g.write_waker);
    }
}

fn broken() -> io::Error {
    io::Error::from(io::ErrorKind::BrokenPipe)
}

pub(crate) struct SendHalf {
    id: u32,
    stream: Arc<Stream>,
    handle: Arc<ConnHandle>,
}

pub(crate) struct RecvHalf {
    id: u32,
    stream: Arc<Stream>,
    handle: Arc<ConnHandle>,
}

pub(super) fn halves(
    id: u32,
    stream: Arc<Stream>,
    handle: Arc<ConnHandle>,
) -> (SendHalf, RecvHalf) {
    let send = SendHalf {
        id,
        stream: stream.clone(),
        handle: handle.clone(),
    };
    (send, RecvHalf { id, stream, handle })
}

impl SendHalf {
    /// Sets `Fin` once; `true` means the stream can now be removed from the table.
    fn finish(&self) -> Option<bool> {
        let mut g = self.stream.lock();
        if g.fin_out {
            return None;
        }
        g.fin_out = true;
        self.handle.shared.send(Frame::Fin(self.id));
        Some(g.done())
    }

    fn finish_and_clean(&self) {
        if self.finish() == Some(true) {
            self.handle.shared.remove(self.id);
        }
    }
}

impl AsyncWrite for SendHalf {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let shared = &self.handle.shared;
        let mut g = self.stream.lock();
        if buf.is_empty() {
            return Poll::Ready(Ok(0));
        }
        if g.fin_out || shared.is_closed() {
            return Poll::Ready(Err(broken()));
        }
        if g.write_credit_bytes == 0 {
            g.write_waker = Some(cx.waker().clone());
            return Poll::Pending;
        }
        let n = buf
            .len()
            .min(g.write_credit_bytes)
            .min(shared.max_packet_bytes() - DATA_FRAME_HEADER_BYTES);
        g.write_credit_bytes -= n;
        shared.send(Frame::Data(self.id, Bytes::copy_from_slice(&buf[..n])));
        Poll::Ready(Ok(n))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if self.handle.shared.is_closed() && !self.stream.lock().fin_out {
            return Poll::Ready(Err(broken()));
        }
        self.finish_and_clean();
        Poll::Ready(Ok(()))
    }
}

impl Drop for SendHalf {
    fn drop(&mut self) {
        if self.handle.shared.is_closed() {
            return;
        }
        self.finish_and_clean();
    }
}

impl AsyncRead for RecvHalf {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        out: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let mut g = self.stream.lock();
        if g.buffered_bytes > 0 {
            g.take(out);
            if let Some(n) = g.window_due() {
                self.handle.shared.send(Frame::Window(self.id, n));
            }
            return Poll::Ready(Ok(()));
        }
        if g.fin_in {
            return Poll::Ready(Ok(()));
        }
        if self.handle.shared.is_closed() {
            return Poll::Ready(Err(io::Error::from(io::ErrorKind::ConnectionAborted)));
        }
        g.read_waker = Some(cx.waker().clone());
        Poll::Pending
    }
}

impl Drop for RecvHalf {
    fn drop(&mut self) {
        let mut g = self.stream.lock();
        g.recv_dropped = true;
        let back = g.buffered_bytes + g.window_owed_bytes;
        g.buf.clear();
        g.buffered_bytes = 0;
        g.window_owed_bytes = 0;
        if back > 0 {
            self.handle.shared.send(Frame::Window(self.id, back as u32));
        }
    }
}
