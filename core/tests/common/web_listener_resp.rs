//! The raw HTTP client and the response parser of the web listener scenarios.
//!
//! What it does: `http_within` sends one request over a plain TCP connection and parses the answer into `Resp`.
//! What it does not do: it does not start the listener (see `web_listener.rs`).
//! Links: included as `#[path = "common/web_listener_resp.rs"] mod resp_common;` by every scenario that sends requests,
//! directly or through the other leaves.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

/// A parsed response: status, the raw head (status line and headers), body.
pub(crate) struct Resp {
    pub(crate) status: u16,
    pub(crate) head: Vec<u8>,
    pub(crate) body: Vec<u8>,
}

impl std::fmt::Debug for Resp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Resp")
            .field("status", &self.status)
            .field("head", &String::from_utf8_lossy(&self.head))
            .field("body_len", &self.body.len())
            .finish()
    }
}

/// The index of the byte right after `\r\n\r\n`, if there is one.
pub(crate) fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| i + 4)
}

pub(crate) fn parse_response(raw_bytes: &[u8]) -> Resp {
    let end = find_header_end(raw_bytes).unwrap_or(raw_bytes.len());
    let head = &raw_bytes[..end];
    let status = String::from_utf8_lossy(head)
        .split("\r\n")
        .next()
        .and_then(|line| line.split_whitespace().nth(1)?.parse().ok())
        .unwrap_or(0);
    Resp {
        status,
        head: head.to_vec(),
        body: raw_bytes[end..].to_vec(),
    }
}

/// `raw` with an explicit timeout: requests with a password hash under load don't fit in 5 s.
pub(crate) fn raw_within(port: u16, bytes: &[u8], limit: Duration) -> Vec<u8> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("raw: connect");
    stream
        .set_read_timeout(Some(Duration::from_millis(300)))
        .ok();
    let _ = stream.write_all(bytes);
    let deadline = Instant::now() + limit;
    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        if Instant::now() >= deadline {
            break;
        }
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => out.extend_from_slice(&buf[..n]),
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                if !out.is_empty() {
                    break;
                }
                continue;
            }
            Err(_) => break,
        }
    }
    out
}

pub(crate) fn http_within(
    port: u16,
    method: &str,
    path: &str,
    extra_headers: &[(&str, &str)],
    body: &[u8],
    limit: Duration,
) -> Resp {
    let has_host = extra_headers
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("host"));
    let has_origin = extra_headers
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("origin"));

    let mut req = format!("{method} {path} HTTP/1.1\r\n");
    if !has_host {
        req.push_str(&format!("Host: 127.0.0.1:{port}\r\n"));
    }
    req.push_str("Connection: close\r\n");
    if !body.is_empty() {
        req.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    let needs_origin = matches!(method, "POST" | "PUT" | "DELETE");
    if needs_origin && !has_origin {
        req.push_str(&format!("Origin: http://127.0.0.1:{port}\r\n"));
    }
    for (k, v) in extra_headers {
        if v.is_empty() {
            continue; // an empty value — the header isn't sent at all
        }
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");

    let mut bytes = req.into_bytes();
    bytes.extend_from_slice(body);
    let raw_bytes = raw_within(port, &bytes, limit);
    parse_response(&raw_bytes)
}
