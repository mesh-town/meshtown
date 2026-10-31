use super::*;
use crate::exit::udpgw::codec::Frame;
use std::io::ErrorKind;

const REQ_DATA_V4: &str = "000e02000101c000020a003570696e67";
const REQ_DATA_DOMAIN: &str = "0014020002030b6578616d706c652e636f6d007b0102";
const REQ_CLOSE_1: &str = "0003040001";
const REQ_KEEPALIVE_7: &str = "0003010007";
const REQ_UNKNOWN_FLAG_9: &str = "0003080009";
const REQ_ERR_4: &str = "0003200004";

fn bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex"))
        .collect()
}

async fn raw(hex: &str) -> RawFrame {
    read_raw(&mut &bytes(hex)[..]).await.expect("frame")
}

#[tokio::test]
async fn reference_frames_have_the_kind_and_id_of_the_exit_order() {
    let cases = [
        (REQ_DATA_V4, Kind::Data, 1),
        (REQ_CLOSE_1, Kind::Close, 1),
        (REQ_KEEPALIVE_7, Kind::Keepalive, 7),
        (REQ_UNKNOWN_FLAG_9, Kind::Other, 9),
        (REQ_ERR_4, Kind::Other, 4),
    ];
    for (hex, kind, id) in cases {
        let frame = raw(hex).await;
        assert_eq!((frame.kind(), frame.conn_id()), (kind, id), "{hex}");
    }
}

#[tokio::test]
async fn keepalive_wins_over_close_and_close_over_data() {
    let flags = |f: u8| RawFrame(vec![0, 3, f, 0, 1]);
    assert_eq!(flags(0x07).kind(), Kind::Keepalive);
    assert_eq!(flags(0x06).kind(), Kind::Close);
    assert_eq!(flags(0x22).kind(), Kind::Data);
    assert_eq!(flags(0x00).kind(), Kind::Other);
}

#[tokio::test]
async fn with_conn_id_changes_only_the_id_bytes() {
    let original = bytes(REQ_DATA_DOMAIN);
    let changed = raw(REQ_DATA_DOMAIN).await.with_conn_id(0x1234).into_bytes();
    assert_eq!(changed.len(), original.len());
    assert_eq!(changed[3..5], [0x12, 0x34]);
    assert_eq!(changed[..3], original[..3]);
    assert_eq!(changed[5..], original[5..]);
}

#[test]
fn own_frames_match_the_codec() {
    let frame = RawFrame::of(&Frame::keepalive(7));
    assert_eq!(frame.into_bytes(), bytes(REQ_KEEPALIVE_7));
    assert_eq!(RawFrame::close(1).into_bytes(), bytes(REQ_CLOSE_1));
}

#[tokio::test]
async fn cut_frame_is_an_error() {
    let cut = bytes(REQ_DATA_V4);
    let err = read_raw(&mut &cut[..cut.len() - 2]).await.expect_err("cut");
    assert_eq!(err.kind(), ErrorKind::UnexpectedEof);
}

#[tokio::test]
async fn body_shorter_than_header_is_invalid() {
    let err = read_raw(&mut &[0u8, 2, 1, 0][..]).await.expect_err("short");
    assert_eq!(err.kind(), ErrorKind::InvalidData);
}

#[tokio::test]
async fn eof_before_a_frame_is_unexpected_eof() {
    let err = read_raw(&mut &[][..]).await.expect_err("eof");
    assert_eq!(err.kind(), ErrorKind::UnexpectedEof);
}
