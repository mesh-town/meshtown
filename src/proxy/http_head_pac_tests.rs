//! Tests of `http_head.rs`: the proxy auto-config file request.

use super::*;

#[test]
fn get_proxy_pac_is_pac_not_proxy() {
    // given
    let raw = b"GET /proxy.pac HTTP/1.1\r\nHost: 127.0.0.1:1080\r\n\r\n";
    // when
    let ask = parse_pac(raw);
    // then
    assert_eq!(ask, PacRequest { head_only: false });
}

#[test]
fn head_proxy_pac_is_head_only() {
    let ask = parse_pac(b"HEAD /proxy.pac HTTP/1.1\r\nHost: 127.0.0.1:1080\r\n\r\n");
    assert_eq!(ask, PacRequest { head_only: true });
}

#[test]
fn other_origin_path_is_still_origin_form_error() {
    let res = parse_head(b"GET / HTTP/1.1\r\nHost: 127.0.0.1:1080\r\n\r\n");
    assert!(
        matches!(res, Err(HeadError::OriginForm)),
        "expected Err(OriginForm)"
    );
}

#[test]
fn unfinished_pac_head_needs_more_bytes() {
    let res = parse_head(b"GET /proxy.pac HTTP/1.1\r\nHost: 127.0.0.1:1080\r\n");
    assert!(matches!(res, Ok(None)), "expected Ok(None)");
}

#[test]
fn absolute_form_pac_is_proxied() {
    // given: browsers fetch the PAC bypassing the proxy, so an absolute URI is ordinary proxying.
    let raw = b"GET http://127.0.0.1:1080/proxy.pac HTTP/1.1\r\nHost: 127.0.0.1:1080\r\n\r\n";
    // when
    let ask = parse_ok(raw);
    // then
    assert_eq!(ask.target, Target::Ip(Ipv4Addr::new(127, 0, 0, 1).into()));
    assert_eq!(ask.port, 1080);
    assert!(
        head_text(&ask).starts_with("GET /proxy.pac HTTP/1.1\r\n"),
        "upstream head: {}",
        head_text(&ask)
    );
}
