//! Tests of `UdpSession` over `tokio::io::duplex` on the paused clock: the idle limit, what keeps a session alive,
//! the end of the stream and the release of the permit.

use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{duplex, split, AsyncRead, AsyncReadExt, AsyncWriteExt, DuplexStream};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;
use tokio::time::Instant;

use super::*;

const IDLE: Duration = Duration::from_secs(30);
const SECOND: Duration = Duration::from_secs(1);

mod empty_read_buffer {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn is_not_reported_as_a_closed_stream() {
        // given: a session whose peer is silent but open
        let (mut session, _peer) = session_over_duplex(IDLE, None);

        // when: it is read into an empty buffer
        let got = session.read(&mut []).await;

        // then: the read returns zero bytes without an error
        assert_eq!(got.unwrap(), 0);
    }
}

mod without_traffic {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn session_is_alive_just_before_the_limit() {
        // given: a session with idle 30 s and a silent peer
        let (session, _peer) = session_over_duplex(IDLE, None);
        let reading = when_reads(session);

        // when: 29 s pass
        pass(IDLE - SECOND).await;

        // then: the read is still waiting
        assert!(
            !reading.is_finished(),
            "the session failed before the limit"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn read_fails_with_timed_out_just_after_the_limit() {
        // given: the same session
        let (session, _peer) = session_over_duplex(IDLE, None);
        let reading = when_reads(session);

        // when: 31 s pass
        pass(IDLE + SECOND).await;

        // then: the read fails with `TimedOut`, the text starts with the mark and says "idle"
        let (read, _) = reading.await.unwrap();
        let err = read.expect_err("an idle session must fail");
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
        assert!(err.to_string().starts_with(UDP_SESSION_MARK), "{err}");
        assert!(err.to_string().contains("idle"), "{err}");
    }

    #[tokio::test(start_paused = true)]
    async fn write_after_the_idle_failure_fails() {
        // given: a session that has failed on idle
        let (session, _peer) = session_over_duplex(IDLE, None);
        let reading = when_reads(session);
        pass(IDLE + SECOND).await;
        let (_, mut session) = reading.await.unwrap();

        // when: something is written to it
        let written = session.write_all(b"late").await;

        // then: the write fails too
        assert!(
            written.is_err(),
            "a write into an idle-closed session succeeded"
        );
    }
}

mod with_traffic {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn incoming_bytes_every_20_seconds_keep_it_alive_for_90() {
        // given: a session with idle 30 s that is being drained
        let (session, mut peer) = session_over_duplex(IDLE, None);
        let (reader, seen) = spawn_drain(session);

        // when: a byte arrives every 20 s for 80 s, then 10 s of silence pass
        for _ in 0..4 {
            pass(Duration::from_secs(20)).await;
            peer.write_all(b"x").await.unwrap();
        }
        pass(Duration::from_secs(10)).await;

        // then: it is alive and has read every byte
        assert!(
            !reader.is_finished(),
            "the session failed while bytes kept arriving"
        );
        assert_eq!(seen.load(Ordering::SeqCst), 4);

        // and: after 31 s of silence it fails on idle
        pass(IDLE + SECOND).await;
        assert_eq!(reader.await.unwrap().kind(), io::ErrorKind::TimedOut);
    }

    #[tokio::test(start_paused = true)]
    async fn outgoing_bytes_keep_it_alive_too() {
        // given: a session split into halves; the read half is waiting, the peer is silent
        let (session, _peer) = session_over_duplex(IDLE, None);
        let (read, mut write) = split(session);
        let (reader, _) = spawn_drain(read);

        // when: a write goes out every 20 s for 80 s, then 10 s pass
        for _ in 0..4 {
            pass(Duration::from_secs(20)).await;
            write.write_all(b"x").await.unwrap();
        }
        pass(Duration::from_secs(10)).await;

        // then: the read half is still waiting
        assert!(
            !reader.is_finished(),
            "writes did not re-arm the idle timer"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn written_bytes_reach_the_peer() {
        // given: a session over a duplex
        let (mut session, mut peer) = session_over_duplex(IDLE, None);

        // when: it writes
        session.write_all(b"hi").await.unwrap();

        // then: the peer reads the same bytes
        let mut buf = [0u8; 2];
        peer.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hi");
    }
}

mod end_of_the_stream {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn remote_close_is_a_read_error_that_says_closed() {
        // given: the remote half is closed
        let (mut session, peer) = session_over_duplex(IDLE, None);
        drop(peer);

        // when: the session is read
        let err = session
            .read(&mut [0u8; 8])
            .await
            .expect_err("EOF must be an error");

        // then: the text starts with the mark and says "closed", not "idle"
        assert!(err.to_string().starts_with(UDP_SESSION_MARK), "{err}");
        assert!(err.to_string().contains("closed"), "{err}");
        assert!(!err.to_string().contains("idle"), "{err}");
    }

    #[tokio::test(start_paused = true)]
    async fn remote_close_does_not_wait_for_the_idle_limit() {
        // given: the remote half is closed
        let (mut session, peer) = session_over_duplex(IDLE, None);
        drop(peer);
        let started = Instant::now();

        // when: the session is read
        let _ = session.read(&mut [0u8; 8]).await;

        // then: the error came at once
        assert!(
            started.elapsed() < SECOND,
            "the end of the stream was reported late"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn bytes_before_the_close_are_still_read() {
        // given: the peer wrote three bytes and closed
        let (mut session, mut peer) = session_over_duplex(IDLE, None);
        peer.write_all(b"abc").await.unwrap();
        drop(peer);

        // when: the session is read twice
        let mut buf = [0u8; 8];
        let first = session.read(&mut buf).await.expect("the bytes come first");
        let second = session.read(&mut buf).await;

        // then: the bytes, then the error
        assert_eq!(&buf[..first], b"abc");
        assert!(second.is_err(), "the end of the stream was not reported");
    }
}

mod permit {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn is_held_while_the_session_lives() {
        // given: a semaphore of one and a session that took its permit
        let semaphore = Arc::new(Semaphore::new(1));
        let permit = semaphore.clone().acquire_owned().await.unwrap();

        // when: the session is alive
        let (_session, _peer) = session_over_duplex(IDLE, Some(permit));

        // then: nothing is left
        assert_eq!(semaphore.available_permits(), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn is_released_when_the_session_is_dropped() {
        // given: a semaphore of one and a session that took its permit
        let semaphore = Arc::new(Semaphore::new(1));
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let (session, _peer) = session_over_duplex(IDLE, Some(permit));

        // when: the session is dropped
        drop(session);

        // then: the permit is back
        assert_eq!(semaphore.available_permits(), 1);
    }
}

/// A session over one half of a duplex pipe and the other half, which plays the remote side.
fn session_over_duplex(
    idle: Duration,
    permit: Option<OwnedSemaphorePermit>,
) -> (UdpSession, DuplexStream) {
    let (near, far) = duplex(1024);
    let (recv, send) = split(near);
    (
        UdpSession::new(Box::new(send), Box::new(recv), idle, permit),
        far,
    )
}

/// A task that reads once from the session and hands the session back with the result.
fn when_reads(mut session: UdpSession) -> JoinHandle<(io::Result<usize>, UdpSession)> {
    crate::node::output::spawn(async move {
        let read = session.read(&mut [0u8; 16]).await;
        (read, session)
    })
}

/// A task that reads until the first error; counts the bytes read.
fn spawn_drain<R: AsyncRead + Unpin + Send + 'static>(
    mut read: R,
) -> (JoinHandle<io::Error>, Arc<AtomicUsize>) {
    let seen = Arc::new(AtomicUsize::new(0));
    let counter = seen.clone();
    let task = crate::node::output::spawn(async move {
        let mut buf = [0u8; 16];
        loop {
            match read.read(&mut buf).await {
                Ok(n) => counter.fetch_add(n, Ordering::SeqCst),
                Err(e) => return e,
            };
        }
    });
    (task, seen)
}

/// Moves the paused clock by `d` and lets the woken tasks run.
async fn pass(d: Duration) {
    tokio::time::advance(d).await;
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
}
