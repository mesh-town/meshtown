//! The publishing side of a UDP session: one stream, one loopback socket connected to the published port.
//!
//! What it does: frames from the stream go to the port as datagrams, the port's replies go back as frames.
//! The socket is bound to `127.0.0.1:0` and connected, so datagrams from other senders never reach the stream.
//! What it does not do: it does not check rights or limits (`server` does) and does not retry a closed port.

use std::io;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::UdpSocket;

use super::pipe::{encode_frame, FrameDecoder};
use crate::node::output::say;

/// A datagram longer than this is cut to it.
const DATAGRAM_LIMIT: usize = 2048;
/// Receive buffer: larger than any UDP datagram.
const RECV_BUF: usize = 64 * 1024;
const STREAM_CHUNK: usize = 16 * 1024;

enum Event {
    Stream(io::Result<usize>),
    Socket(io::Result<usize>),
}

/// Serves one stream until it ends or breaks, or the loopback socket fails.
pub(crate) async fn serve_udp_stream<S>(stream: S, port: u16)
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let socket = match connected_socket(port).await {
        Ok(socket) => socket,
        Err(e) => return say("port", format!("udp {port}: local socket failed: {e}")),
    };
    let (mut rd, mut wr) = tokio::io::split(stream);
    pump(&mut rd, &mut wr, &socket, port).await;
    let _ = wr.shutdown().await;
}

async fn connected_socket(port: u16) -> io::Result<UdpSocket> {
    let socket = UdpSocket::bind(("127.0.0.1", 0)).await?;
    socket.connect(("127.0.0.1", port)).await?;
    Ok(socket)
}

async fn next_event<R: AsyncRead + Unpin>(
    rd: &mut R,
    socket: &UdpSocket,
    chunk: &mut [u8],
    dgram: &mut [u8],
) -> Event {
    tokio::select! {
        n = rd.read(chunk) => Event::Stream(n),
        n = socket.recv(dgram) => Event::Socket(n),
    }
}

/// A stream write does not wait for an ACK per datagram; `flush` runs only when idle, so a tail held
/// by the transport does not sit until the next datagram.
async fn pump<R, W>(rd: &mut R, wr: &mut W, socket: &UdpSocket, port: u16)
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut decoder = FrameDecoder::default();
    let (mut chunk, mut dgram) = (vec![0u8; STREAM_CHUNK], vec![0u8; RECV_BUF]);
    let (mut unflushed, mut send_failed) = (false, false);
    loop {
        let event = tokio::select! {
            biased;
            event = next_event(rd, socket, &mut chunk, &mut dgram) => event,
            flushed = wr.flush(), if unflushed => {
                unflushed = false;
                if flushed.is_err() {
                    return;
                }
                continue;
            }
        };
        let go_on = match event {
            Event::Stream(Ok(n)) if n > 0 => {
                forward_to_port(socket, port, &mut decoder, &chunk[..n], &mut send_failed).await
            }
            Event::Socket(got) => {
                unflushed = true;
                forward_to_stream(wr, port, got, &dgram).await
            }
            Event::Stream(_) => false,
        };
        if !go_on {
            return;
        }
    }
}

/// Frames from the stream become datagrams; a refusing port is reported once and does not end the session.
async fn forward_to_port(
    socket: &UdpSocket,
    port: u16,
    decoder: &mut FrameDecoder,
    chunk: &[u8],
    send_failed: &mut bool,
) -> bool {
    for body in decoder.push(chunk) {
        match socket.send(&body).await {
            Err(e) if !*send_failed => {
                *send_failed = true;
                say("port", format!("udp {port}: send failed: {e}"));
            }
            _ => {}
        }
    }
    true
}

/// A datagram from the port becomes a frame; a refusal reported by an earlier send is not an end.
async fn forward_to_stream<W: AsyncWrite + Unpin>(
    wr: &mut W,
    port: u16,
    got: io::Result<usize>,
    dgram: &[u8],
) -> bool {
    match got {
        Ok(n) => wr.write_all(&encode_frame(&dgram[..n.min(DATAGRAM_LIMIT)])).await.is_ok(),
        Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => true,
        Err(e) => {
            say("port", format!("udp {port}: local socket error: {e}"));
            false
        }
    }
}

#[cfg(test)]
#[path = "serve_tests.rs"]
mod serve_tests;
