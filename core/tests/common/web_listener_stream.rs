//! One response off an open stream, for the scenarios whose connection outlives a request.
//!
//! What it does: `read_one_response` reads by `Content-Length` and leaves the stream open.
//! What it does not do: it does not open connections (see `web_listener_resp.rs`, which it takes `Resp` and the parser from).
//! Links: included as a sibling of `common` by the scenarios that need it; takes the base through `super::common`.

use std::io::Read;
use std::net::TcpStream;

use super::resp_common::{find_header_end, parse_response, Resp};

/// Reads exactly one HTTP response from an already open stream (by `Content-Length`) without closing it —
/// needed where the connection lives longer than one request. A missing `Content-Length` in the headers
/// (not our case there, but just in case) is treated as the end of the headers with no body.
pub(crate) fn read_one_response(stream: &mut TcpStream) -> Resp {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        let n = stream.read(&mut chunk).expect("read_one_response: read");
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(end) = find_header_end(&buf) {
            let head = String::from_utf8_lossy(&buf[..end]).to_lowercase();
            let content_length = head
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .and_then(|v| v.trim().parse::<usize>().ok());
            match content_length {
                Some(len) => {
                    if buf.len() >= end + len {
                        break;
                    }
                }
                None => break,
            }
        }
    }
    parse_response(&buf)
}
