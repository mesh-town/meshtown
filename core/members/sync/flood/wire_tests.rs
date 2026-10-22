//! Tests for `SYNC` frames: write-read round trip, length limit, garbage, end of stream.

use super::{read_frame, write_frame, Frame, MAX_FRAME_BYTES, VERSION};
use crate::transport::net::error::NetError;
use crate::transport::net::stream::{BoxRecv, BoxSend};
use tokio::io::{duplex, split, AsyncWriteExt};

/// A pipe: what is written to `BoxSend` is read from `BoxRecv`.
fn pipe() -> (BoxSend, BoxRecv) {
    let (a, b) = duplex(1 << 20);
    let (_ar, aw) = split(a);
    let (br, _bw) = split(b);
    (Box::new(aw), Box::new(br))
}

mod frame {
    use super::*;

    /// given: one frame of each kind → when: write and read through the pipe → then: what is read equals the original.
    #[tokio::test]
    async fn roundtrip_each_kind() {
        let kinds = [
            Frame::Hello {
                version: VERSION,
                topic: [1; 32],
                tag: [2; 32],
            },
            Frame::Welcome { tag: [3; 32] },
            Frame::Msg {
                ttl: 6,
                body: b"hello".to_vec(),
            },
            Frame::Ping,
        ];
        let (mut send, mut recv) = pipe();

        for frame in kinds {
            write_frame(&mut send, &frame).await.expect("write");
            let got = read_frame(&mut recv).await.expect("read");
            assert_eq!(got, frame);
        }
    }

    /// given: a header with length `MAX_FRAME_BYTES + 1` → when: `read_frame` → then: `Io(InvalidData)`.
    #[tokio::test]
    async fn rejects_length_over_max() {
        let (mut send, mut recv) = pipe();
        send.write_all(&(MAX_FRAME_BYTES as u32 + 1).to_be_bytes())
            .await
            .expect("header");

        let err = read_frame(&mut recv).await.expect_err("too long");

        assert!(is_invalid_data(&err), "expected Io(InvalidData), got {err}");
    }

    /// given: garbage of the right length → when: `read_frame` → then: `Io(InvalidData)`.
    #[tokio::test]
    async fn garbage_body_is_invalid() {
        let (mut send, mut recv) = pipe();
        send.write_all(&4u32.to_be_bytes()).await.expect("length");
        send.write_all(&[0xff; 4]).await.expect("garbage");

        let err = read_frame(&mut recv).await.expect_err("garbage");

        assert!(is_invalid_data(&err), "expected Io(InvalidData), got {err}");
    }

    /// given: the stream is closed before a frame and mid-frame → when: `read_frame` → then: `Closed` both times.
    #[tokio::test]
    async fn eof_is_closed() {
        let (send, mut recv) = pipe();
        drop(send);
        let before = read_frame(&mut recv).await.expect_err("end of stream");
        assert!(
            matches!(before, NetError::Closed),
            "before the frame: {before}"
        );

        let (mut send, mut recv) = pipe();
        send.write_all(&10u32.to_be_bytes()).await.expect("length");
        send.write_all(&[1, 2, 3]).await.expect("part of the body");
        drop(send);
        let mid = read_frame(&mut recv).await.expect_err("break");
        assert!(matches!(mid, NetError::Closed), "mid-frame: {mid}");
    }

    fn is_invalid_data(err: &NetError) -> bool {
        matches!(err, NetError::Io(e) if e.kind() == std::io::ErrorKind::InvalidData)
    }
}
