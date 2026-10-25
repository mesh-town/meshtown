//! Tests for `Accept-Encoding` parsing and unpacking of the embedded gzip bodies.

use axum::http::{header, HeaderMap, HeaderValue};

use super::{accepts_gzip, inflate_gzip};
use crate::web_gui::routes::pages::PAGES;

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

fn index_gz() -> Vec<u8> {
    PAGES
        .iter()
        .find(|e| e.0 == "/")
        .expect("index entry")
        .2
        .to_vec()
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

mod inflate_gzip_body {
    use super::*;

    #[test]
    fn inflates_the_index_entry() {
        // given
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/web_gui/ui/index.html");
        // when
        let out = inflate_gzip(&index_gz()).expect("inflate");
        // then
        assert_eq!(out, std::fs::read(path).unwrap());
    }

    #[test]
    fn rejects_bad_magic_short_input_and_flags() {
        // given
        let mut bad_magic = index_gz();
        bad_magic[1] = 0x8c;
        let mut flagged = index_gz();
        flagged[3] = 0x08;
        let inputs = [vec![], index_gz()[..17].to_vec(), bad_magic, flagged];
        // then
        for input in inputs {
            assert!(inflate_gzip(&input).is_err(), "{} bytes", input.len());
        }
    }

    #[test]
    fn rejects_isize_mismatch() {
        // given
        let mut gz = index_gz();
        *gz.last_mut().unwrap() ^= 0x01;
        // then
        assert!(inflate_gzip(&gz).is_err());
    }
}
