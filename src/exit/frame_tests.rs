//! Tests for the exit protocol frame: building and parsing a request, reply codes (`frame.rs`).

use super::*;

#[test]
fn request_roundtrip() {
    let raw = encode_exit_request("Example.COM", 443).unwrap();
    assert_eq!(raw[0], EXIT_PROTOCOL_VERSION);
    assert_eq!(usize::from(raw[1]), "Example.COM".len());
    let (domain, port) = parse_exit_request(&raw).unwrap();
    assert_eq!(domain, "example.com");
    assert_eq!(port, 443);
}

#[test]
fn request_with_long_name_is_rejected() {
    let long = "a".repeat(256);
    assert!(encode_exit_request(&long, 80).is_err());
    assert!(encode_exit_request("", 80).is_err());
}

#[test]
fn parse_rejects_short_buffer() {
    assert!(parse_exit_request(&[EXIT_PROTOCOL_VERSION, 1, b'a']).is_err());
    // The name length is larger than the number of bytes in the frame.
    assert!(parse_exit_request(&[EXIT_PROTOCOL_VERSION, 9, b'a', 0, 80]).is_err());
}

#[test]
fn parse_rejects_other_version() {
    let mut raw = encode_exit_request("example.com", 80).unwrap();
    raw[0] = 2;
    assert!(parse_exit_request(&raw).is_err());
}

#[test]
fn every_code_has_text() {
    for code in [
        EXIT_CODE_OK,
        EXIT_CODE_NOT_IN_LIST,
        EXIT_CODE_NO_ADDRESS,
        EXIT_CODE_CONNECT_FAILED,
        EXIT_CODE_PORT_NOT_ALLOWED,
        EXIT_CODE_REFUSED,
        EXIT_CODE_BAD_REQUEST,
        EXIT_CODE_ADDRESS_FORBIDDEN,
    ] {
        assert_ne!(exit_code_text(code), "unknown reply code");
    }
    assert_eq!(exit_code_text(200), "unknown reply code");
}

#[test]
fn not_in_list_names_the_network() {
    assert!(exit_code_text(EXIT_CODE_NOT_IN_LIST).contains(crate::output::NET_NAME));
}

#[test]
fn address_forbidden_is_new_code() {
    let earlier = [
        EXIT_CODE_OK,
        EXIT_CODE_NOT_IN_LIST,
        EXIT_CODE_NO_ADDRESS,
        EXIT_CODE_CONNECT_FAILED,
        EXIT_CODE_PORT_NOT_ALLOWED,
        EXIT_CODE_REFUSED,
        EXIT_CODE_BAD_REQUEST,
    ];

    assert_eq!(
        EXIT_CODE_ADDRESS_FORBIDDEN, 7,
        "the code number is part of the protocol: old clients know 0..=6"
    );
    assert!(
        !earlier.contains(&EXIT_CODE_ADDRESS_FORBIDDEN),
        "code 7 must not coincide with any earlier one"
    );
    assert!(
        exit_code_text(EXIT_CODE_ADDRESS_FORBIDDEN).contains("closed"),
        "the refusal text says the address is closed: {}",
        exit_code_text(EXIT_CODE_ADDRESS_FORBIDDEN)
    );
}
