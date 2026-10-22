use super::*;

// ==============================================================================================
// parse_* — on raw bytes (10)
// ==============================================================================================

#[test]
fn parse_200_with_length_ok() {
    let raw = b"HTTP/1.0 200 OK\r\nContent-Length: 5\r\n\r\nhello";
    let (status, body) = parse_response(raw).expect("must parse");
    assert_eq!(status, 200);
    assert_eq!(body, b"hello");
}

#[test]
fn parse_200_read_until_close_without_length() {
    let raw = b"HTTP/1.0 200 OK\r\n\r\nhello world";
    let (status, body) = parse_response(raw).expect("without Content-Length the whole rest is the body");
    assert_eq!(status, 200);
    assert_eq!(body, b"hello world");
}

#[test]
fn parse_http11_ok() {
    let raw = b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\n\r\n";
    let (status, body) = parse_response(raw).expect("HTTP/1.1 parses too");
    assert_eq!(status, 202);
    assert!(body.is_empty());
}

#[test]
fn parse_lowercase_header_names_ok() {
    let raw = b"HTTP/1.0 200 OK\r\ncontent-length: 5\r\n\r\nhello";
    let (status, body) = parse_response(raw).expect("header names are case-insensitive");
    assert_eq!(status, 200);
    assert_eq!(body, b"hello");
}

#[test]
fn parse_body_shorter_than_content_length_is_broken() {
    let raw = b"HTTP/1.0 200 OK\r\nContent-Length: 10\r\n\r\nabc";
    let err = parse_response(raw).expect_err("the body is shorter than the declared length");
    assert!(matches!(err, ApiError::NotHttp(_)));
}

#[test]
fn parse_missing_header_terminator_is_not_http() {
    let raw = b"HTTP/1.0 200 OK\r\nContent-Length: 5";
    let err = parse_response(raw).expect_err("no end of headers");
    assert!(matches!(err, ApiError::NotHttp(_)));
}

#[test]
fn parse_header_over_16kib_is_not_http() {
    let mut raw = b"HTTP/1.0 200 OK\r\n".to_vec();
    raw.extend(std::iter::repeat_n(b'a', 20 * 1024));
    let err = parse_response(&raw).expect_err("a header larger than 16 KiB without an end");
    assert!(matches!(err, ApiError::NotHttp(_)));
}

#[test]
fn parse_body_over_1mib_is_not_http() {
    let raw = b"HTTP/1.0 200 OK\r\nContent-Length: 2000000\r\n\r\n".to_vec();
    let err = parse_response(&raw).expect_err("the declared body is larger than 1 MiB");
    assert!(matches!(err, ApiError::NotHttp(_)));
}

#[test]
fn parse_non_http_banner_is_not_http() {
    let raw = b"SSH-2.0-x\r\n\r\n";
    let err = parse_response(raw).expect_err("a banner is not an HTTP response");
    assert!(matches!(err, ApiError::NotHttp(_)));
}

#[test]
fn parse_transfer_encoding_is_not_http() {
    let raw = b"HTTP/1.0 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n";
    let err = parse_response(raw).expect_err("Transfer-Encoding is not supported");
    assert!(matches!(err, ApiError::NotHttp(_)));
}
