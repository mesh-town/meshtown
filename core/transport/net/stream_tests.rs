//! Tests for `read_to_end_limited` (`net/stream.rs`): reading a stream to the end with a length limit.
//! The stream is the reading half of `tokio::io::duplex` into which the data was written beforehand.

use super::*;
use std::io::ErrorKind;
use std::time::Duration;
use tokio::io::{AsyncWriteExt, DuplexStream};

/// A length that clearly exceeds the size of one read: the stream arrives in several chunks.
const LARGE: usize = 100_000;

mod stream_of_exactly_the_limit {
    use super::*;

    #[tokio::test]
    async fn is_read_in_full() {
        // given: a 10-byte stream closed by the writing side
        let data = bytes(10);
        let mut recv = finished(&data).await;

        // when: we read with a limit of 10
        let got = read_to_end_limited(&mut recv, 10).await;

        // then: all 10 bytes are received
        assert_eq!(got.expect("exactly the limit is not an error"), data);
    }

    #[tokio::test]
    async fn spanning_many_reads_keeps_every_byte_in_order() {
        // given: a stream of LARGE bytes with a non-repeating pattern
        let data = bytes(LARGE);
        let mut recv = finished(&data).await;

        // when: we read with a limit of LARGE
        let got = read_to_end_limited(&mut recv, LARGE).await;

        // then: the same bytes in the same order
        assert_eq!(got.expect("exactly the limit is not an error"), data);
    }
}

mod stream_over_the_limit {
    use super::*;

    #[tokio::test]
    async fn one_byte_over_is_invalid_data() {
        // given: an 11-byte stream
        let mut recv = finished(&bytes(11)).await;

        // when: we read with a limit of 10
        let got = read_to_end_limited(&mut recv, 10).await;

        // then: an InvalidData error
        assert_invalid_data(got);
    }

    #[tokio::test]
    async fn spanning_many_reads_one_byte_over_is_invalid_data() {
        // given: a stream of LARGE + 1 bytes
        let mut recv = finished(&bytes(LARGE + 1)).await;

        // when: we read with a limit of LARGE
        let got = read_to_end_limited(&mut recv, LARGE).await;

        // then: an InvalidData error
        assert_invalid_data(got);
    }

    #[tokio::test]
    async fn limit_zero_with_a_single_byte_is_invalid_data() {
        // given: a one-byte stream
        let mut recv = finished(&bytes(1)).await;

        // when: we read with a limit of 0
        let got = read_to_end_limited(&mut recv, 0).await;

        // then: an InvalidData error
        assert_invalid_data(got);
    }

    #[tokio::test]
    async fn unfinished_stream_fails_without_waiting_for_its_end() {
        // The guard against an unbounded stream does not wait for its end: like the earlier QUIC `read_to_end(limit)`,
        // the refusal comes as soon as more than the limit has been read.
        // given: the writing side sent limit + 1 bytes and is silent, not closing the stream
        let (_writer, mut recv) = written(&bytes(11)).await;

        // when: we read with a limit of 10 under a deadline
        let got = tokio::time::timeout(Duration::from_secs(2), read_to_end_limited(&mut recv, 10))
            .await
            .expect(
                "the read waited for the end of the stream although the limit is already exceeded",
            );

        // then: an InvalidData error
        assert_invalid_data(got);
    }
}

mod empty_stream {
    use super::*;

    #[tokio::test]
    async fn gives_an_empty_vec() {
        // given: a stream with no data, closed by the writing side
        let mut recv = finished(&[]).await;

        // when: we read with a limit of 10
        let got = read_to_end_limited(&mut recv, 10).await;

        // then: an empty vector
        assert_eq!(
            got.expect("an empty stream is not an error"),
            Vec::<u8>::new()
        );
    }

    #[tokio::test]
    async fn with_limit_zero_gives_an_empty_vec() {
        // given: a stream with no data, closed by the writing side
        let mut recv = finished(&[]).await;

        // when: we read with a limit of 0
        let got = read_to_end_limited(&mut recv, 0).await;

        // then: an empty vector
        assert_eq!(
            got.expect("an empty stream is not an error"),
            Vec::<u8>::new()
        );
    }
}

/// `len` bytes of a non-repeating pattern: a lost or swapped byte is noticeable.
fn bytes(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

/// A stream into which `data` is written, with the writing side alive and not closed: there is no end of stream.
async fn written(data: &[u8]) -> (DuplexStream, BoxRecv) {
    let (mut writer, reader) = tokio::io::duplex(data.len().max(1));
    writer.write_all(data).await.expect("write to duplex");
    let reader: BoxRecv = Box::new(reader);
    (writer, reader)
}

/// A stream into which `data` is written, with the writing side closed: the reader will reach the end of the stream.
async fn finished(data: &[u8]) -> BoxRecv {
    let (mut writer, reader) = written(data).await;
    writer.shutdown().await.expect("closing the writing side");
    reader
}

fn assert_invalid_data(got: std::io::Result<Vec<u8>>) {
    let err = got.expect_err("an error was expected over the limit");
    assert_eq!(err.kind(), ErrorKind::InvalidData, "expected InvalidData");
}
