//! Tests for the common frame: round trip, length limit, end of stream.

use super::*;
use tokio::io::{duplex, split, AsyncWriteExt};

type Pipe = (
    tokio::io::WriteHalf<tokio::io::DuplexStream>,
    tokio::io::ReadHalf<tokio::io::DuplexStream>,
);

fn pipe() -> Pipe {
    let (a, b) = duplex(1 << 20);
    let (_ar, aw) = split(a);
    let (br, _bw) = split(b);
    (aw, br)
}

fn is_invalid(e: &NetError) -> bool {
    matches!(e, NetError::Io(io) if io.kind() == ErrorKind::InvalidData)
}

#[tokio::test]
async fn roundtrip_generic() {
    let (mut w, mut r) = pipe();
    let msg = (7u32, vec![1u8, 2, 3], "text".to_string());
    write_framed(&mut w, &msg, 1024).await.unwrap();
    let got: (u32, Vec<u8>, String) = read_framed(&mut r, 1024).await.unwrap();
    assert_eq!(got, msg);
}

#[tokio::test]
async fn over_max_is_invalid_data() {
    let (mut w, mut r) = pipe();
    let big = vec![9u8; 100];
    let err = write_framed(&mut w, &big, 50).await.unwrap_err();
    assert!(is_invalid(&err), "write over the limit: {err}");
    write_framed(&mut w, &big, 1000).await.unwrap();
    let err = read_framed::<_, Vec<u8>>(&mut r, 50).await.unwrap_err();
    assert!(is_invalid(&err), "read over the limit: {err}");
}

#[tokio::test]
async fn eof_is_closed() {
    let (mut w, mut r) = pipe();
    w.write_all(&[0, 0]).await.unwrap();
    drop(w);
    let err = read_framed::<_, Vec<u8>>(&mut r, 50).await.unwrap_err();
    assert!(matches!(err, NetError::Closed), "{err}");
}
