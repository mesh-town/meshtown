use super::*;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Accepts every write into `written`; `flush` never completes.
struct NeverFlushes {
    written: Vec<u8>,
}

impl AsyncWrite for NeverFlushes {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.written.extend_from_slice(buf);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Pending
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

/// Never accepts a write.
struct StuckWrite;

impl AsyncWrite for StuckWrite {
    fn poll_write(self: Pin<&mut Self>, _: &mut Context<'_>, _: &[u8]) -> Poll<io::Result<usize>> {
        Poll::Pending
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Pending
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Pending
    }
}

#[tokio::test]
async fn write_one_does_not_wait_for_flush() {
    let mut w = NeverFlushes {
        written: Vec::new(),
    };
    let done = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        write_one(&mut w, &Ctrl::Ping),
    )
    .await;
    assert!(done.expect("write_one waited for flush").is_ok());
    assert!(!w.written.is_empty());
}

#[tokio::test(start_paused = true)]
async fn write_one_still_times_out_on_a_stuck_write() {
    let started = Instant::now();
    let err = write_one(&mut StuckWrite, &Ctrl::Ping).await.unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::TimedOut);
    assert!(started.elapsed() >= DEAD_AFTER);
}
