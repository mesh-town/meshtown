//! Headers and frames on the wire: a "write → read" round trip and format rejections.

use super::*;
use std::io::ErrorKind;

async fn header_bytes(h: &Header) -> Vec<u8> {
    let mut buf = Vec::new();
    write_header(&mut buf, h).await.unwrap();
    buf
}

async fn header_round(h: Header) -> Header {
    let buf = header_bytes(&h).await;
    read_header(&mut buf.as_slice()).await.unwrap()
}

#[tokio::test]
async fn open_round_trips() {
    let h = Header::Open { session: 0x0102_0304_0506_0708, service_name: b"auth".to_vec() };
    assert_eq!(header_round(h.clone()).await, h);
}

#[tokio::test]
async fn stream_round_trips() {
    for owner in [Owner::Dialer, Owner::Acceptor] {
        let h = Header::Stream { session: 42, owner };
        assert_eq!(header_round(h.clone()).await, h);
    }
}

#[tokio::test]
async fn header_starts_with_magic_and_is_big_endian() {
    let buf = header_bytes(&Header::Stream { session: 1, owner: Owner::Acceptor }).await;
    assert_eq!(&buf[..4], b"mvh1");
    assert_eq!(&buf[buf.len() - 9..buf.len() - 1], &1u64.to_be_bytes());
    assert_eq!(buf[buf.len() - 1], 1);
}

#[tokio::test]
async fn ack_and_refuse_round_trip() {
    let replies = [Reply::Ack, Reply::Refuse { code: REFUSE_UNSERVED, reason: b"no such service".to_vec() }];
    for r in replies {
        let mut buf = Vec::new();
        write_reply(&mut buf, &r).await.unwrap();
        assert_eq!(read_reply(&mut buf.as_slice()).await.unwrap(), r);
    }
}

#[tokio::test]
async fn refuse_round_trips() {
    let r = Reply::Refuse { code: REFUSE_NO_SESSION, reason: Vec::new() };
    let mut buf = Vec::new();
    write_reply(&mut buf, &r).await.unwrap();
    assert_eq!(read_reply(&mut buf.as_slice()).await.unwrap(), r);
}

#[tokio::test]
async fn close_frame_round_trips() {
    let frames = [
        Ctrl::Close { code: REFUSE_SESSION_CLOSED, reason: "bye".as_bytes().to_vec() },
        Ctrl::Ping,
        Ctrl::Pong,
    ];
    let mut buf = Vec::new();
    for f in &frames {
        write_ctrl(&mut buf, f).await.unwrap();
    }
    let mut src = buf.as_slice();
    for f in frames {
        assert_eq!(read_ctrl(&mut src).await.unwrap(), f);
    }
}

#[tokio::test]
async fn foreign_magic_is_invalid_data() {
    let mut buf = header_bytes(&Header::Stream { session: 1, owner: Owner::Dialer }).await;
    buf[0] = b'X';
    let err = read_header(&mut buf.as_slice()).await.unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidData);
}

#[tokio::test]
async fn unknown_kind_is_invalid_data() {
    let mut buf = b"mvh1".to_vec();
    buf.push(99);
    let err = read_header(&mut buf.as_slice()).await.unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidData);
    let err = read_ctrl(&mut [0x77u8].as_slice()).await.unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidData);
    let err = read_reply(&mut [9u8].as_slice()).await.unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidData);
}

#[tokio::test]
async fn unknown_owner_is_invalid_data() {
    let mut buf = header_bytes(&Header::Stream { session: 1, owner: Owner::Dialer }).await;
    *buf.last_mut().unwrap() = 7;
    let err = read_header(&mut buf.as_slice()).await.unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidData);
}

#[tokio::test]
async fn service_longer_than_64_is_rejected() {
    let h = Header::Open { session: 1, service_name: vec![b'a'; 65] };
    let err = write_header(&mut Vec::new(), &h).await.unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidData);
    let mut buf = b"mvh1".to_vec();
    buf.push(1);
    buf.extend_from_slice(&1u64.to_be_bytes());
    buf.push(65);
    buf.extend(vec![b'a'; 65]);
    let err = read_header(&mut buf.as_slice()).await.unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidData);
}

#[tokio::test]
async fn service_of_exactly_64_is_accepted() {
    let h = Header::Open { session: 1, service_name: vec![b'a'; 64] };
    assert_eq!(header_round(h.clone()).await, h);
}

#[tokio::test]
async fn reason_longer_than_255_is_rejected_on_write() {
    let long = vec![b'r'; 256];
    let err = write_reply(&mut Vec::new(), &Reply::Refuse { code: 1, reason: long.clone() }).await;
    assert_eq!(err.unwrap_err().kind(), ErrorKind::InvalidData);
    let err = write_ctrl(&mut Vec::new(), &Ctrl::Close { code: 1, reason: long }).await;
    assert_eq!(err.unwrap_err().kind(), ErrorKind::InvalidData);
}

#[tokio::test]
async fn eof_inside_header_is_unexpected_eof() {
    let buf = header_bytes(&Header::Open { session: 5, service_name: b"auth".to_vec() }).await;
    for cut in [0, 2, 6, buf.len() - 1] {
        let err = read_header(&mut &buf[..cut]).await.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnexpectedEof, "break at {cut}");
    }
}

#[test]
fn constants_match_the_plan() {
    assert_eq!(MAGIC, *b"mvh1");
    assert_eq!(HEADER_LIMIT, std::time::Duration::from_secs(5));
    assert_eq!((REFUSE_UNSERVED, REFUSE_NO_SESSION, REFUSE_SESSION_CLOSED), (1, 2, 3));
}
