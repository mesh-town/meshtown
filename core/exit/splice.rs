//! Shuttling bytes between a QUIC stream and a TCP connection — shared by both sides of
//! the exit protocol (on the exit `tcp` is the connection to the domain, on the requester's side it is to the program).

use std::time::Duration;

use super::splice_progress::{wait_second_half, Counted, Progress};
use super::windowed_copy::copy_windowed;
use crate::transport::net::{BoxRecv, BoxSend};
use tokio::net::TcpStream;

/// How long the other side may stay idle after the first one has closed (a busy one is not cut).
pub const HALF_CLOSE_GRACE: Duration = Duration::from_secs(60);

/// Idle period for a stream with no bytes on the first side to close (browser pre-connections: 0/0 bytes):
/// there is no one to give data to on the other side. It starts only after one side's FIN.
pub const HALF_CLOSE_GRACE_SILENT: Duration = Duration::from_secs(5);

/// The wait for the other side: 0 bytes on the first closed side — `HALF_CLOSE_GRACE_SILENT`, otherwise `HALF_CLOSE_GRACE`.
pub(super) fn half_close_grace_after(first_bytes: u64) -> Duration {
    match first_bytes {
        0 => HALF_CLOSE_GRACE_SILENT,
        _ => HALF_CLOSE_GRACE,
    }
}

/// Guard of the two `splice` subtasks: while alive it holds their
/// `AbortHandle`s; on its own `Drop` it aborts both subtasks. Without it, cancelling `splice()` itself
/// (dropping its future from outside, for example `select!` in `splice_while_chosen`) doesn't
/// reach the client — the TCP socket halves keep living in `quic_to_tcp_task`/`tcp_to_quic_task`, and a `JoinHandle` doesn't
/// abort its task on `drop`. On normal completion (both subtasks already removed by hand per the
/// code below or run to the end) a repeated `abort()` on a finished task is a no-op,
/// the behavior for the second caller (`exit::server`, the exit side) doesn't change.
struct AbortBothOnDrop {
    quic_to_tcp_task: tokio::task::AbortHandle,
    tcp_to_quic_task: tokio::task::AbortHandle,
}

impl Drop for AbortBothOnDrop {
    fn drop(&mut self) {
        self.quic_to_tcp_task.abort();
        self.tcp_to_quic_task.abort();
    }
}

/// Shuttles bytes between a QUIC stream and a TCP connection.
///
/// Returns a pair of numbers for the proxy's summary line: how many bytes came
/// from the QUIC stream into TCP (for the requester, "received from the site") and how many went from TCP
/// into the QUIC stream ("sent to the site"). The numbers are what `tokio::io::copy` returned. A direction
/// cut by `abort` yields no number and counts as zero — in the log this shows as "0 bytes" for the
/// unfinished direction. The cut comes `HALF_CLOSE_GRACE` after the second half last made progress
/// (not after the first half closed); the `Counted` wrappers only tell whether bytes moved. Dropping
/// the `splice()` future itself removes both subtasks through `AbortBothOnDrop`. No cap on the bytes
/// toward the QUIC stream.
pub async fn splice(send: BoxSend, recv: BoxRecv, tcp: TcpStream) -> (u64, u64) {
    splice_windowed(send, recv, tcp, 0).await
}

/// `splice` with a cap on the bytes written into the QUIC stream before waiting for them to be
/// acknowledged: `window` is that cap in bytes; 0 — no cap.
pub(crate) async fn splice_windowed(
    send: BoxSend,
    mut recv: BoxRecv,
    tcp: TcpStream,
    window: usize,
) -> (u64, u64) {
    // The trait is needed for shutdown on the TCP half and on the sending half of the stream.
    use tokio::io::AsyncWriteExt;

    let (mut tcp_read, tcp_write) = tcp.into_split();
    let (to_tcp, to_quic) = (Progress::default(), Progress::default());
    let mut tcp_write = Counted::new(tcp_write, to_tcp.clone());
    let mut send = Counted::new(send, to_quic.clone());
    let mut quic_to_tcp_task = crate::node::output::spawn(async move {
        let n = tokio::io::copy(&mut recv, &mut tcp_write)
            .await
            .unwrap_or(0);
        let _ = tcp_write.shutdown().await;
        n
    });
    let mut tcp_to_quic_task = crate::node::output::spawn(async move {
        let n = copy_windowed(&mut tcp_read, &mut send, window)
            .await
            .unwrap_or(0);
        let _ = send.shutdown().await;
        n
    });
    let _guard = AbortBothOnDrop {
        quic_to_tcp_task: quic_to_tcp_task.abort_handle(),
        tcp_to_quic_task: tcp_to_quic_task.abort_handle(),
    };
    tokio::select! {
        first = &mut quic_to_tcp_task => {
            let quic_to_tcp_bytes = first.unwrap_or(0);
            let tcp_to_quic_bytes =
                wait_second_half(&mut tcp_to_quic_task, &to_quic, quic_to_tcp_bytes).await;
            tcp_to_quic_task.abort();
            (quic_to_tcp_bytes, tcp_to_quic_bytes)
        }
        first = &mut tcp_to_quic_task => {
            let tcp_to_quic_bytes = first.unwrap_or(0);
            let quic_to_tcp_bytes =
                wait_second_half(&mut quic_to_tcp_task, &to_tcp, tcp_to_quic_bytes).await;
            quic_to_tcp_task.abort();
            (quic_to_tcp_bytes, tcp_to_quic_bytes)
        }
    }
}

#[cfg(test)]
#[path = "splice_tests.rs"]
mod tests;
