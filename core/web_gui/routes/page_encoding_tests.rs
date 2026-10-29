//! Tests for `Accept-Encoding` parsing.

use axum::http::{header, HeaderMap, HeaderValue};

use super::accepts_gzip;

fn lines(values: &[&str]) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for v in values {
        headers.append(header::ACCEPT_ENCODING, HeaderValue::from_str(v).unwrap());
    }
    headers
}

fn accepts(value: &str) -> bool {
    accepts_gzip(&lines(&[value]))
}

mod accepts_gzip_header {
    use super::*;

    #[test]
    fn browser_list_accepts_gzip() {
        assert!(accepts("gzip, deflate, br"));
    }

    #[test]
    fn missing_header_refuses_gzip() {
        assert!(!accepts_gzip(&HeaderMap::new()));
    }

    #[test]
    fn identity_only_refuses_gzip() {
        assert!(!accepts("identity"));
    }

    #[test]
    fn gzip_q_zero_is_a_refusal() {
        // given
        let refusals = ["gzip;q=0", "gzip; q=0.000", "gzip;q=0, *"];
        // then
        for v in refusals {
            assert!(!accepts(v), "{v}");
        }
        assert!(accepts("gzip;q=0.001"));
    }

    #[test]
    fn wildcard_accepts_gzip() {
        assert!(accepts("*"));
        assert!(!accepts("*;q=0"));
    }

    #[test]
    fn case_and_spaces_are_ignored() {
        assert!(accepts("deflate , GZIP;q=0.5"));
    }

    #[test]
    fn any_header_line_counts() {
        assert!(accepts_gzip(&lines(&["identity", "gzip"])));
    }

    #[test]
    fn x_gzip_lookalike_is_not_gzip() {
        assert!(!accepts("x-gzip2"));
    }
}
