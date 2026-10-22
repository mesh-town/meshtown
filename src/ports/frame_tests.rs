//! Tests of the `meshtown/port/1` protocol frame (`frame.rs`).

use super::*;

const CODES: [u8; 5] = [
    PORT_CODE_OK,
    PORT_CODE_NOT_PUBLISHED,
    PORT_CODE_CONNECT_FAILED,
    PORT_CODE_REFUSED,
    PORT_CODE_BAD_REQUEST,
];

#[test]
fn request_roundtrip_and_layout() {
    assert_eq!(encode_port_request(8080), [1, 0x1F, 0x90]);
    assert_eq!(parse_port_request(&encode_port_request(8080)).unwrap(), 8080);
}

#[test]
fn reply_roundtrip() {
    for code in CODES {
        assert_eq!(parse_port_reply(&encode_port_reply(code)).unwrap(), code);
    }
}

#[test]
fn parse_rejects() {
    assert!(parse_port_request(&[2, 0x1F, 0x90]).is_err());
    assert!(parse_port_reply(&[2, 0]).is_err());
    assert!(parse_port_request(&[1, 0x1F]).is_err());
    assert!(parse_port_request(&[1, 0x1F, 0x90, 0]).is_err());
    assert!(parse_port_reply(&[1]).is_err());
    assert!(parse_port_reply(&[1, 0, 0]).is_err());
    assert!(parse_port_request(&[1, 0, 0]).is_err());
}

#[test]
fn parse_reply_passes_unknown_code() {
    assert_eq!(parse_port_reply(&[1, 9]).unwrap(), 9);
}

#[test]
fn code_texts_distinct() {
    let unknown = port_code_text(9);
    let texts: Vec<&str> = CODES.iter().map(|c| port_code_text(*c)).collect();
    for (i, t) in texts.iter().enumerate() {
        assert!(!t.is_empty());
        assert_ne!(*t, unknown);
        assert!(texts[i + 1..].iter().all(|o| o != t), "duplicate text {t}");
    }
}
