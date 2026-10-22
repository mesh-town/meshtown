//! Tests of the pure parsing of an HTTP request head to the proxy (`http_head.rs`).

use super::*;
use crate::proxy::pac::PacRequest;
use crate::socks5::Target;
use std::net::{Ipv4Addr, Ipv6Addr};

#[path = "http_head_ipv6_tests.rs"]
mod connect_ipv6;
#[path = "http_head_pac_tests.rs"]
mod pac_file;

mod connect {
    use super::*;

    mod with_port {
        use super::*;

        #[test]
        fn targets_domain_and_port_with_ok_reply_and_empty_upstream_head() {
            // given
            let raw = b"CONNECT site.test:443 HTTP/1.1\r\nHost: site.test:443\r\n\r\n";
            // when
            let ask = parse_ok(raw);
            // then
            assert_eq!(ask.target, Target::Domain("site.test".to_string()));
            assert_eq!(ask.port, 443);
            assert_eq!(ask.reply, CONNECT_OK);
            assert!(ask.upstream_head.is_empty(), "CONNECT has no upstream head");
        }
    }

    mod ipv4_without_port {
        use super::*;

        #[test]
        fn targets_ip_and_port_443() {
            let ask = parse_ok(b"CONNECT 10.0.0.1 HTTP/1.1\r\n\r\n");
            assert_eq!(ask.target, Target::Ip(Ipv4Addr::new(10, 0, 0, 1).into()));
            assert_eq!(ask.port, 443);
        }
    }

    mod early_bytes_after_head {
        use super::*;

        #[test]
        fn head_len_points_at_first_tls_byte() {
            let head = b"CONNECT site.test:443 HTTP/1.1\r\n\r\n";
            let mut raw = head.to_vec();
            raw.extend_from_slice(b"\x16\x03\x01");
            let ask = parse_ok(&raw);
            assert_eq!(ask.head_len, head.len());
            assert_eq!(raw[ask.head_len], 0x16);
        }
    }

    mod bad_port {
        use super::*;

        #[test]
        fn zero_is_rejected() {
            assert!(parse_err(b"CONNECT site.test:0 HTTP/1.1\r\n\r\n"));
        }

        #[test]
        fn not_a_number_is_rejected() {
            assert!(parse_err(b"CONNECT site.test:x HTTP/1.1\r\n\r\n"));
        }
    }
}

mod forward {
    use super::*;

    mod absolute_uri_with_port_and_query {
        use super::*;

        #[test]
        fn head_is_rewritten_to_origin_form_with_host_from_uri() {
            let ask = parse_ok(b"GET http://Site.test:8080/a?b=1 HTTP/1.1\r\nHost: x\r\n\r\n");
            let head = head_text(&ask);
            assert!(
                head.starts_with("GET /a?b=1 HTTP/1.1\r\n"),
                "head: {head:?}"
            );
            assert!(
                head.contains("\r\nHost: site.test:8080\r\n"),
                "head: {head:?}"
            );
            assert_eq!(ask.target, Target::Domain("site.test".to_string()));
            assert_eq!(ask.port, 8080);
            assert!(
                ask.reply.is_empty(),
                "forwarding has no reply to the client"
            );
        }
    }

    mod empty_path_default_port {
        use super::*;

        #[test]
        fn path_becomes_slash_and_host_has_no_port() {
            let ask = parse_ok(b"GET http://site.test HTTP/1.1\r\n\r\n");
            let head = head_text(&ask);
            assert!(head.starts_with("GET / HTTP/1.1\r\n"), "head: {head:?}");
            assert!(head.contains("\r\nHost: site.test\r\n"), "head: {head:?}");
            assert_eq!(ask.port, 80);
        }
    }

    mod hop_by_hop_headers {
        use super::*;

        #[test]
        fn are_dropped_others_keep_order_and_connection_close_is_last() {
            let raw = b"GET http://site.test/ HTTP/1.1\r\nProxy-Connection: keep-alive\r\n\
proxy-authorization: Basic eDp5\r\nConnection: keep-alive\r\nKeep-Alive: 5\r\n\
Host: old.test\r\nX-A: 1\r\nX-B: 2\r\n\r\n";
            let ask = parse_ok(raw);
            let head = head_text(&ask);
            let lower = head.to_ascii_lowercase();
            assert!(!lower.contains("proxy-connection"), "head: {head:?}");
            assert!(!lower.contains("proxy-authorization"), "head: {head:?}");
            assert!(!lower.contains("keep-alive"), "head: {head:?}");
            assert!(
                !lower.contains("old.test"),
                "the old Host remained: {head:?}"
            );
            assert!(!lower.contains("connection: keep"), "head: {head:?}");
            let a = head.find("X-A: 1").expect("X-A lost");
            let b = head.find("X-B: 2").expect("X-B lost");
            assert!(a < b, "header order broken: {head:?}");
            assert!(
                head.ends_with("Connection: close\r\n\r\n"),
                "the last header is Connection: close: {head:?}"
            );
        }
    }

    mod body_in_same_buffer {
        use super::*;

        #[test]
        fn head_len_points_at_first_body_byte() {
            let head = b"POST http://site.test/p HTTP/1.1\r\nContent-Length: 5\r\n\r\n";
            let mut raw = head.to_vec();
            raw.extend_from_slice(b"hello");
            let ask = parse_ok(&raw);
            assert_eq!(ask.head_len, head.len());
            assert_eq!(raw[ask.head_len], b'h');
        }

        #[test]
        fn body_beyond_max_head_is_not_an_error() {
            let head = b"POST http://site.test/p HTTP/1.1\r\nContent-Length: 70000\r\n\r\n";
            let mut raw = head.to_vec();
            raw.extend(std::iter::repeat_n(b'z', MAX_HEAD_BYTES + 10));
            let ask = parse_ok(&raw);
            assert_eq!(ask.head_len, head.len());
        }
    }

    mod scheme_case {
        use super::*;

        #[test]
        fn uppercase_http_is_accepted_with_port_80() {
            let ask = parse_ok(b"GET HTTP://site.test/ HTTP/1.1\r\n\r\n");
            assert_eq!(ask.port, 80);
        }
    }

    mod not_absolute_http {
        use super::*;

        #[test]
        fn origin_form_is_rejected() {
            assert!(parse_err(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n"));
        }

        #[test]
        fn https_scheme_is_rejected() {
            assert!(parse_err(b"GET https://site.test/ HTTP/1.1\r\n\r\n"));
        }
    }
}

mod incomplete_or_oversized {
    use super::*;

    #[test]
    fn head_without_terminator_needs_more_bytes() {
        let res = parse_head(b"GET http://site.test/ HTTP/1.1\r\nHost: x\r\n");
        assert!(matches!(res, Ok(None)), "expected Ok(None)");
    }

    #[test]
    fn head_over_max_head_without_end_is_rejected() {
        let raw = vec![b'a'; MAX_HEAD_BYTES + 1];
        assert!(parse_err(&raw));
    }

    #[test]
    fn hundred_and_one_headers_are_rejected() {
        let mut raw = b"GET http://site.test/ HTTP/1.1\r\n".to_vec();
        for _ in 0..101 {
            raw.extend_from_slice(b"X-N: 1\r\n");
        }
        raw.extend_from_slice(b"\r\n");
        assert!(parse_err(&raw));
    }
}

mod status_reply {
    use super::*;

    #[test]
    fn bad_gateway_has_status_line_exact_length_and_connection_close() {
        // given
        let body = "address not found: éøü";
        // when
        let reply = String::from_utf8(status_reply(502, body)).unwrap();
        // then
        assert!(
            reply.starts_with("HTTP/1.1 502 Bad Gateway\r\n"),
            "reply: {reply:?}"
        );
        let (head, tail) = reply.split_once("\r\n\r\n").expect("no end of head");
        assert!(
            head.contains(&format!("Content-Length: {}", tail.len())),
            "Content-Length is the body length in UTF-8 bytes ({} bytes): {reply:?}",
            tail.len()
        );
        assert!(
            tail.len() > body.chars().count(),
            "multibyte text: more bytes than characters"
        );
        assert!(head.contains("Connection: close"), "reply: {reply:?}");
        assert!(tail.starts_with(body), "the body is the reason: {reply:?}");
    }

    #[test]
    fn reason_phrases_match_codes() {
        for (code, phrase) in [(400, "Bad Request"), (504, "Gateway Timeout")] {
            let reply = String::from_utf8(status_reply(code, "x")).unwrap();
            assert!(
                reply.starts_with(&format!("HTTP/1.1 {code} {phrase}\r\n")),
                "{reply:?}"
            );
        }
    }
}

fn parse_ok(raw: &[u8]) -> HttpProxyRequest {
    match parse_head(raw) {
        Ok(Some(HttpRequestKind::Proxy(ask))) => ask,
        Ok(Some(HttpRequestKind::Pac(_))) => panic!(
            "expected proxying, got PAC: {:?}",
            String::from_utf8_lossy(raw)
        ),
        Ok(None) => panic!(
            "head considered incomplete: {:?}",
            String::from_utf8_lossy(raw)
        ),
        Err(e) => panic!("unexpected parse refusal: {e}"),
    }
}

fn parse_pac(raw: &[u8]) -> PacRequest {
    match parse_head(raw) {
        Ok(Some(HttpRequestKind::Pac(ask))) => ask,
        Ok(Some(HttpRequestKind::Proxy(_))) => panic!(
            "expected PAC, got proxying: {:?}",
            String::from_utf8_lossy(raw)
        ),
        Ok(None) => panic!(
            "head considered incomplete: {:?}",
            String::from_utf8_lossy(raw)
        ),
        Err(e) => panic!("unexpected parse refusal: {e}"),
    }
}

fn parse_err(raw: &[u8]) -> bool {
    parse_head(raw).is_err()
}

fn head_text(ask: &HttpProxyRequest) -> String {
    String::from_utf8(ask.upstream_head.clone()).expect("upstream head is UTF-8")
}
