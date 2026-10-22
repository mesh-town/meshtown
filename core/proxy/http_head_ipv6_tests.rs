//! Tests of `http_head.rs`: CONNECT and absolute URIs with IPv6 literals.

use super::*;

#[test]
fn connect_to_bracketed_literal_is_parsed() {
    let ask = parse_ok(b"CONNECT [::1]:443 HTTP/1.1\r\n\r\n");
    assert_eq!(ask.target, Target::Ip(Ipv6Addr::LOCALHOST.into()));
    assert_eq!(ask.port, 443);
}

#[test]
fn absolute_uri_with_bracketed_literal_keeps_host_header() {
    let ask = parse_ok(b"GET http://[2001:db8::1]:8080/x HTTP/1.1\r\nHost: other\r\n\r\n");
    assert_eq!(
        ask.target,
        Target::Ip("2001:db8::1".parse::<Ipv6Addr>().unwrap().into())
    );
    assert_eq!(ask.port, 8080);
    assert!(
        head_text(&ask).contains("Host: [2001:db8::1]:8080\r\n"),
        "upstream head: {}",
        head_text(&ask)
    );
}

#[test]
fn bracketed_literal_without_port_uses_default() {
    let ask = parse_ok(b"GET http://[2001:db8::1]/ HTTP/1.1\r\n\r\n");
    assert_eq!(ask.port, 80);
    assert!(
        head_text(&ask).contains("Host: [2001:db8::1]\r\n"),
        "upstream head: {}",
        head_text(&ask)
    );
}

#[test]
fn unclosed_bracket_is_rejected() {
    assert!(parse_err(b"CONNECT [::1:443 HTTP/1.1\r\n\r\n"));
}

#[test]
fn unbracketed_ipv6_is_rejected() {
    assert!(parse_err(b"CONNECT ::1:443 HTTP/1.1\r\n\r\n"));
}
