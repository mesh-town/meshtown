//! Tests of `copy_windowed` (`windowed_copy.rs`): the cap on bytes written but not yet acknowledged, the flush
//! at the end of the stream and on the reader's pause, failures and cancellation. The writer is the stub of
//! `windowed_copy_stub.rs`, whose `flush` waits for an acknowledgement like the transport's does.

use super::stub::{ack_writer, AckMode, AckWriter};
use super::*;
use std::io::{self, Cursor, ErrorKind};
use std::time::Duration;
use tokio::io::{duplex, AsyncWriteExt};
use tokio::task::JoinHandle;

const KIB: usize = 1024;
const WINDOW: usize = 64 * KIB;
const PATIENCE: Duration = Duration::from_secs(5);
const QUIET: Duration = Duration::from_millis(50);

mod without_window {
    use super::*;

    #[tokio::test]
    async fn zero_window_is_plain_copy_with_one_flush_at_eof() {
        // given: 256 KiB to copy into a writer that acknowledges at once
        let data = pattern(256 * KIB);
        let (mut writer, acks) = ack_writer(AckMode::Auto);
        let mut reader = data.as_slice();

        // when: copying with the window 0
        let copied = copy_windowed(&mut reader, &mut writer, 0).await;

        // then: every byte arrived, the only flush is the one at the end, and nothing held the data back
        assert_eq!(copied.expect("copy"), 256 * KIB as u64);
        assert_eq!(acks.written(), data, "the bytes arrive unchanged");
        assert_eq!(acks.flushes(), 1, "no flush before the end of the stream");
        assert_eq!(
            acks.max_unacked(),
            256 * KIB,
            "without a window all of it may be unacknowledged"
        );
    }
}

mod with_window {
    use super::*;

    #[tokio::test]
    async fn never_more_than_window_unacknowledged() {
        // given: 1 MiB to copy with a window of 64 KiB into a writer that acknowledges at every flush
        let data = pattern(MIB);
        let (mut writer, acks) = ack_writer(AckMode::Auto);
        let mut reader = data.as_slice();

        // when: copying
        let copied = copy_windowed(&mut reader, &mut writer, WINDOW).await;

        // then: all bytes arrived, and at no moment more than the window was waiting for an acknowledgement
        assert_eq!(copied.expect("copy"), MIB as u64);
        assert_eq!(acks.written(), data, "the bytes arrive unchanged");
        assert!(
            acks.max_unacked() <= WINDOW,
            "{} bytes were unacknowledged at once",
            acks.max_unacked()
        );
        assert!(
            acks.flushes() >= 16,
            "1 MiB in 64 KiB windows needs a flush per window, got {}",
            acks.flushes()
        );
    }

    #[tokio::test]
    async fn stops_at_the_window_until_acked() {
        // given: 1 MiB being copied with a window of 64 KiB into a writer that waits for acknowledgements
        let (writer, acks) = ack_writer(AckMode::Manual);
        let copy = start_copy(pattern(MIB), writer, WINDOW);

        // when: nothing is acknowledged
        acks.wait_written(WINDOW, PATIENCE).await;
        tokio::time::sleep(QUIET).await;

        // then: the copy stands exactly on the window and is still alive
        assert_eq!(acks.written_len(), WINDOW, "the copy went past the window");
        assert!(!copy.is_finished(), "the copy must wait, not end");

        // when: the writer acknowledges
        acks.ack_all();

        // then: the next window is let through
        acks.wait_written(2 * WINDOW, PATIENCE).await;
        copy.abort();
    }

    #[tokio::test]
    async fn one_byte_window_moves_every_byte() {
        // given: 1000 bytes and a window of one byte
        let data = pattern(1000);
        let (mut writer, acks) = ack_writer(AckMode::Auto);
        let mut reader = data.as_slice();

        // when: copying
        let copied = copy_windowed(&mut reader, &mut writer, 1).await;

        // then: every byte arrived, one at a time
        assert_eq!(copied.expect("copy"), 1000);
        assert_eq!(acks.written(), data, "the bytes arrive unchanged");
        assert!(
            acks.max_unacked() <= 1,
            "{} unacknowledged",
            acks.max_unacked()
        );
    }

    #[tokio::test]
    async fn exactly_one_window_waits_only_at_eof() {
        // given: a stream exactly as long as the window, into a writer that waits for acknowledgements
        let (writer, acks) = ack_writer(AckMode::Manual);
        let mut copy = start_copy(pattern(WINDOW), writer, WINDOW);

        // when: all of it is written and nothing is acknowledged
        acks.wait_written(WINDOW, PATIENCE).await;
        tokio::time::sleep(QUIET).await;

        // then: the copy waits for the flush at the end of the stream
        assert!(
            !copy.is_finished(),
            "the final flush must wait for the acknowledgement"
        );

        // when: the acknowledgement comes
        acks.ack_all();

        // then: the copy ends with the full count
        assert_eq!(finished(&mut copy).await.expect("copy"), WINDOW as u64);
    }

    #[tokio::test]
    async fn one_byte_past_the_window_waits_for_one_ack() {
        // given: a stream one byte longer than the window, into a writer that waits for acknowledgements
        let (writer, acks) = ack_writer(AckMode::Manual);
        let mut copy = start_copy(pattern(WINDOW + 1), writer, WINDOW);

        // when: nothing is acknowledged
        acks.wait_written(WINDOW, PATIENCE).await;
        tokio::time::sleep(QUIET).await;

        // then: the extra byte is held back
        assert_eq!(
            acks.written_len(),
            WINDOW,
            "the extra byte went past the window"
        );

        // when: one acknowledgement
        acks.ack_all();

        // then: the byte goes through, and then the copy waits for the flush at the end
        acks.wait_written(WINDOW + 1, PATIENCE).await;
        tokio::time::sleep(QUIET).await;
        assert!(
            !copy.is_finished(),
            "the final flush must wait for the acknowledgement"
        );
        acks.ack_all();
        assert_eq!(finished(&mut copy).await.expect("copy"), WINDOW as u64 + 1);
    }

    #[tokio::test]
    async fn empty_reader_returns_zero() {
        // given: nothing to copy
        let (mut writer, acks) = ack_writer(AckMode::Auto);
        let mut reader: &[u8] = &[];

        // when: copying with a window
        let copied = copy_windowed(&mut reader, &mut writer, WINDOW).await;

        // then: zero bytes, and the end of the stream is flushed once
        assert_eq!(copied.expect("copy"), 0);
        assert_eq!(acks.flushes(), 1);
        assert_eq!(acks.written_len(), 0);
    }

    #[tokio::test]
    async fn reader_pause_flushes_and_restarts_the_window() {
        // given: a site that sent 10 bytes and went quiet, into a writer that waits for acknowledgements
        let (mut site, reader) = duplex(2 * WINDOW);
        let (writer, acks) = ack_writer(AckMode::Manual);
        let copy = start_copy_from(reader, writer, WINDOW);
        site.write_all(&[7u8; 10]).await.expect("site write");

        // when: the copy catches up with the pause
        acks.wait_written(10, PATIENCE).await;
        acks.wait_flushes(1, PATIENCE).await;

        // then: it asked for the acknowledgement of what it had written (the flush of the pause went through the cap)
        assert!(acks.flushes() >= 1, "no flush on the reader's pause");

        // when: the acknowledgement comes and the flush ends
        let before = acks.flushes();
        acks.ack_all();
        acks.wait_flushes(before + 1, PATIENCE).await;

        // then: the count of unacknowledged bytes started over, so a full window follows the 10 bytes
        site.write_all(&vec![8u8; WINDOW])
            .await
            .expect("site write");
        acks.wait_written(10 + WINDOW, PATIENCE).await;
        tokio::time::sleep(QUIET).await;
        assert_eq!(
            acks.written_len(),
            10 + WINDOW,
            "the window did not restart after the flush"
        );
        copy.abort();
    }
}

mod failures {
    use super::*;

    #[tokio::test]
    async fn flush_error_stops_the_copy() {
        // given: a writer whose flush fails, and 256 KiB to copy with a window of 64 KiB
        let data = pattern(256 * KIB);
        let (mut writer, acks) = ack_writer(AckMode::Auto);
        acks.fail_flush(ErrorKind::BrokenPipe);
        let mut reader = data.as_slice();

        // when: copying
        let copied = copy_windowed(&mut reader, &mut writer, WINDOW).await;

        // then: the copy ends with that error right after the first window
        let err = copied.expect_err("the flush error must come out");
        assert_eq!(err.kind(), ErrorKind::BrokenPipe);
        assert_eq!(
            acks.written_len(),
            WINDOW,
            "nothing is written after the failed flush"
        );
    }

    #[tokio::test]
    async fn write_error_is_returned() {
        // given: a writer whose write fails
        let (mut writer, acks) = ack_writer(AckMode::Auto);
        acks.fail_write(ErrorKind::ConnectionReset);
        let data = pattern(1000);
        let mut reader = data.as_slice();

        // when: copying with a window
        let copied = copy_windowed(&mut reader, &mut writer, WINDOW).await;

        // then: the copy ends with that error
        let err = copied.expect_err("the write error must come out");
        assert_eq!(err.kind(), ErrorKind::ConnectionReset);
    }

    #[tokio::test]
    async fn abort_while_waiting_for_ack_ends_at_once() {
        // given: a copy standing on a full window and waiting for an acknowledgement
        let (writer, acks) = ack_writer(AckMode::Manual);
        let copy = start_copy(pattern(MIB), writer, WINDOW);
        acks.wait_written(WINDOW, PATIENCE).await;
        tokio::time::sleep(QUIET).await;
        assert!(!copy.is_finished(), "the copy must be waiting");

        // when: its task is aborted
        copy.abort();

        // then: the task ends as cancelled within a second
        let ended = tokio::time::timeout(Duration::from_secs(1), copy).await;
        let joined = ended.expect("the aborted copy must end at once");
        assert!(joined.expect_err("cancelled, not finished").is_cancelled());
    }
}

// ---- setup and helpers ----

const MIB: usize = 1024 * KIB;

type Copy = JoinHandle<io::Result<u64>>;

/// `len` bytes that don't repeat within a window, so a lost or shifted byte shows.
fn pattern(len: usize) -> Vec<u8> {
    (0..len)
        .map(|i| u8::try_from(i % 251).unwrap_or(0))
        .collect()
}

/// A copy of `data` into `writer` running in its own task.
fn start_copy(data: Vec<u8>, writer: AckWriter, window: usize) -> Copy {
    start_copy_from(Cursor::new(data), writer, window)
}

/// A copy from any reader into `writer` running in its own task.
fn start_copy_from<R>(mut reader: R, mut writer: AckWriter, window: usize) -> Copy
where
    R: tokio::io::AsyncRead + Send + Unpin + 'static,
{
    crate::node::output::spawn(async move { copy_windowed(&mut reader, &mut writer, window).await })
}

/// The result of a copy task, within a step.
async fn finished(copy: &mut Copy) -> io::Result<u64> {
    crate::transport::net::facade::test_support::within("copy finished", copy)
        .await
        .expect("the copy task didn't panic")
}
