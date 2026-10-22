//! The request bytes verbatim: method, path, `Host`/`Origin`, the token (only if it consists of
//! `[0-9A-Za-z]`), the body length.

/// The request bytes verbatim. A `token` that is empty or has a character outside `[0-9A-Za-z]` does not go into the
/// headers — there is no `X-Meshtown-Token` line.
pub(crate) fn request_bytes_without_body(method: &str, path: &str, port: u16, token: &str) -> Vec<u8> {
    let host = host_header(port);
    let mut out = format!("{method} {path} HTTP/1.0\r\nHost: {host}\r\n");
    if method == "POST" {
        out.push_str(&format!("Origin: http://{host}\r\n"));
    }
    if token_is_well_formed(token) {
        out.push_str(&format!("X-Meshtown-Token: {token}\r\n"));
    }
    if method == "POST" {
        out.push_str("Content-Length: 0\r\n");
    } else {
        out.push_str("Accept: application/json\r\n");
    }
    out.push_str("\r\n");
    out.into_bytes()
}

/// The bytes of a `PUT` request with a body (`PUT /api/config`) — like `request_bytes_without_body`, but with `Origin`
/// always (guard.rs requires it on PUT) and a body instead of `Content-Length: 0`.
pub(crate) fn request_bytes_put(path: &str, port: u16, token: &str, body: &[u8]) -> Vec<u8> {
    let host = host_header(port);
    let mut out = format!("PUT {path} HTTP/1.0\r\nHost: {host}\r\nOrigin: http://{host}\r\n");
    if token_is_well_formed(token) {
        out.push_str(&format!("X-Meshtown-Token: {token}\r\n"));
    }
    out.push_str(&format!("Content-Length: {}\r\n\r\n", body.len()));
    let mut out = out.into_bytes();
    out.extend_from_slice(body);
    out
}

/// `Host`/`Origin` as in `guard.rs:32–36`: on port 80 — without the port.
fn host_header(port: u16) -> String {
    if port == 80 {
        "127.0.0.1".to_string()
    } else {
        format!("127.0.0.1:{port}")
    }
}

fn token_is_well_formed(token: &str) -> bool {
    !token.is_empty() && token.bytes().all(|b| b.is_ascii_alphanumeric())
}

#[cfg(test)]
#[path = "request_tests.rs"]
mod tests;
