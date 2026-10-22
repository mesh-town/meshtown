//! Shuttling bytes between a QUIC stream and a TCP connection — shared by both sides of
//! the exit protocol (on the exit `tcp` is the connection to the domain, on the requester's side it is to the program).

use std::time::Duration;

use crate::net::{BoxRecv, BoxSend};
use tokio::net::TcpStream;
use tokio::time::timeout;

/// How long to wait for the other side after the first one has closed.
pub const HALF_CLOSE_GRACE: Duration = Duration::from_secs(60);

/// Grace period for a stream with no bytes on the first side to close (browser pre-connections: 0/0 bytes):
/// there is no one to give data to on the other side. It starts only after one side's FIN.
pub const HALF_CLOSE_GRACE_SILENT: Duration = Duration::from_secs(5);

/// The wait for the other side: 0 bytes on the first closed side — `HALF_CLOSE_GRACE_SILENT`, otherwise `HALF_CLOSE_GRACE`.
fn half_close_grace_after(first_bytes: u64) -> Duration {
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
/// into the QUIC stream ("sent to the site"). Both numbers come from what
/// `tokio::io::copy` returned; there are no separate counters. A direction that was cut by `abort`
/// after `HALF_CLOSE_GRACE` expired yields no number and counts as zero — in the log this shows
/// as "0 bytes" for the unfinished direction. Dropping the `splice()` future itself removes
/// both subtasks through `AbortBothOnDrop`.
pub async fn splice(mut send: BoxSend, mut recv: BoxRecv, tcp: TcpStream) -> (u64, u64) {
    // The trait is needed for shutdown on the TCP half and on the sending half of the stream.
    use tokio::io::AsyncWriteExt;

    let (mut tcp_read, mut tcp_write) = tcp.into_split();
    let mut quic_to_tcp_task = tokio::spawn(async move {
        let n = tokio::io::copy(&mut recv, &mut tcp_write)
            .await
            .unwrap_or(0);
        let _ = tcp_write.shutdown().await;
        n
    });
    let mut tcp_to_quic_task = tokio::spawn(async move {
        let n = tokio::io::copy(&mut tcp_read, &mut send).await.unwrap_or(0);
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
            let tcp_to_quic_bytes = timeout(half_close_grace_after(quic_to_tcp_bytes), &mut tcp_to_quic_task)
                .await
                .ok()
                .and_then(|joined| joined.ok())
                .unwrap_or(0);
            tcp_to_quic_task.abort();
            (quic_to_tcp_bytes, tcp_to_quic_bytes)
        }
        first = &mut tcp_to_quic_task => {
            let tcp_to_quic_bytes = first.unwrap_or(0);
            let quic_to_tcp_bytes = timeout(half_close_grace_after(tcp_to_quic_bytes), &mut quic_to_tcp_task)
                .await
                .ok()
                .and_then(|joined| joined.ok())
                .unwrap_or(0);
            quic_to_tcp_task.abort();
            (quic_to_tcp_bytes, tcp_to_quic_bytes)
        }
    }
}

#[cfg(test)]
#[path = "splice_tests.rs"]
mod tests;
