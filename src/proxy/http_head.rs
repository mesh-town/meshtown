//! Parsing the head of an HTTP request to the proxy: `CONNECT host:port`, a request with an absolute URI `http://…`
//! and `GET`/`HEAD /proxy.pac`.
//! Pure functions without I/O; the reply to the client and the connection are in `http.rs`.

use std::fmt;

use crate::socks5::Target;

use super::pac::{pac_request, PacRequest};

/// Limit on the request head: if it is not read up to it, refusal.
pub(super) const MAX_HEAD_BYTES: usize = 64 * 1024;
/// Reply to a successful `CONNECT`.
pub(super) const CONNECT_OK: &[u8] = b"HTTP/1.1 200 Connection established\r\n\r\n";
const MAX_HEADER_COUNT: usize = 100;
/// Headers the proxy does not forward upstream.
const DROPPED_REQUEST_HEADERS: [&str; 5] = [
    "host",
    "proxy-connection",
    "proxy-authorization",
    "connection",
    "keep-alive",
];

/// A parsed head: where to connect, what to reply to the client and what to send upstream first.
pub(super) struct HttpProxyRequest {
    pub target: Target,
    pub port: u16,
    /// `CONNECT` — `CONNECT_OK`; forwarding — empty.
    pub reply: &'static [u8],
    /// `CONNECT` — empty; forwarding — the rewritten head.
    pub upstream_head: Vec<u8>,
    /// Bytes of the head in the client buffer.
    pub head_len: usize,
}

/// What the client asked for: proxying or the PAC file.
pub(super) enum HttpRequestKind {
    Proxy(HttpProxyRequest),
    Pac(PacRequest),
}

/// Why the head was not accepted; the text goes out as the body of the `400` reply.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum HeadError {
    Syntax(String),
    TooLarge,
    TooManyHeaders,
    NotHttpScheme,
    OriginForm,
    BadAuthority,
}

impl fmt::Display for HeadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HeadError::Syntax(e) => write!(f, "request not parsed: {e}"),
            HeadError::TooLarge => write!(f, "request head is longer than {MAX_HEAD_BYTES} bytes"),
            HeadError::TooManyHeaders => write!(f, "more than {MAX_HEADER_COUNT} headers"),
            HeadError::NotHttpScheme => write!(f, "only http:// and CONNECT are supported"),
            HeadError::OriginForm => write!(f, "request to the proxy without an absolute address"),
            HeadError::BadAuthority => write!(
                f,
                "target address not parsed (bad host or port; an IPv6 literal needs brackets)"
            ),
        }
    }
}

/// Parses the head. `Ok(None)` — the head is not fully read, the buffer is smaller than `MAX_HEAD_BYTES`.
pub(super) fn parse_head(buf: &[u8]) -> Result<Option<HttpRequestKind>, HeadError> {
    let mut headers = [httparse::EMPTY_HEADER; MAX_HEADER_COUNT];
    let mut req = httparse::Request::new(&mut headers);
    let head_len = match req.parse(buf) {
        Ok(httparse::Status::Complete(n)) => n,
        Ok(httparse::Status::Partial) => return partial(buf),
        Err(httparse::Error::TooManyHeaders) => return Err(HeadError::TooManyHeaders),
        Err(e) => return Err(HeadError::Syntax(e.to_string())),
    };
    if head_len > MAX_HEAD_BYTES {
        return Err(HeadError::TooLarge);
    }
    let method = req.method.unwrap_or("");
    let request_target = req.path.unwrap_or("");
    if let Some(pac) = pac_request(method, request_target) {
        return Ok(Some(HttpRequestKind::Pac(pac)));
    }
    if method.eq_ignore_ascii_case("CONNECT") {
        let (target, port) = split_authority(request_target, 443)?;
        return Ok(Some(HttpRequestKind::Proxy(HttpProxyRequest {
            target,
            port,
            reply: CONNECT_OK,
            upstream_head: Vec::new(),
            head_len,
        })));
    }
    absolute_form_request(method, request_target, req.version, req.headers, head_len)
        .map(|request| Some(HttpRequestKind::Proxy(request)))
}

/// A request with an absolute URI: the target comes from the URI, the head is rewritten for the server.
fn absolute_form_request(
    method: &str,
    request_target: &str,
    version: Option<u8>,
    headers: &[httparse::Header],
    head_len: usize,
) -> Result<HttpProxyRequest, HeadError> {
    let (authority, origin) = split_absolute(request_target)?;
    let (target, port) = split_authority(authority, 80)?;
    let name = authority_host(authority).to_lowercase();
    let host = if port == 80 {
        name
    } else {
        format!("{name}:{port}")
    };
    let version = if version == Some(0) {
        "HTTP/1.0"
    } else {
        "HTTP/1.1"
    };
    let upstream_head = rewrite(method, &origin, version, &host, headers);
    Ok(HttpProxyRequest {
        target,
        port,
        reply: b"",
        upstream_head,
        head_len,
    })
}

fn partial(buf: &[u8]) -> Result<Option<HttpRequestKind>, HeadError> {
    if buf.len() >= MAX_HEAD_BYTES {
        return Err(HeadError::TooLarge);
    }
    Ok(None)
}

/// Splits an absolute URI `http://authority/path?query` into authority and origin-form.
fn split_absolute(uri: &str) -> Result<(&str, String), HeadError> {
    let Some((scheme, rest)) = uri.split_once("://") else {
        return Err(HeadError::OriginForm);
    };
    if !scheme.eq_ignore_ascii_case("http") {
        return Err(HeadError::NotHttpScheme);
    }
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    let origin = match tail.chars().next() {
        None | Some('#') => "/".to_string(),
        Some('?') => format!("/{tail}"),
        _ => tail.to_string(),
    };
    Ok((authority, origin))
}

/// Host name from `host`, `host:port`, `[v6]` or `[v6]:port` (as written in the URI); a bracketed literal keeps
/// its brackets.
fn authority_host(authority: &str) -> &str {
    match authority.strip_prefix('[') {
        Some(rest) => rest
            .find(']')
            .map_or(authority, |end| &authority[..end + 2]),
        None => authority.split(':').next().unwrap_or(authority),
    }
}

/// `host[:port]` or `[v6][:port]` → target and port; without a port — `default_port`. A bare IPv6 literal
/// (without brackets) is ambiguous with the port and is refused.
fn split_authority(authority: &str, default_port: u16) -> Result<(Target, u16), HeadError> {
    let (host, port_text) = split_host_port(authority)?;
    let port = port_text.map_or(Ok(default_port), |text| {
        text.parse::<u16>().map_err(|_| HeadError::BadAuthority)
    })?;
    let target = Target::from_host(host).ok_or(HeadError::BadAuthority)?;
    if port == 0 {
        return Err(HeadError::BadAuthority);
    }
    Ok((target, port))
}

/// Splits the authority into the host (an IPv6 literal without brackets) and the port text, if any.
fn split_host_port(authority: &str) -> Result<(&str, Option<&str>), HeadError> {
    let Some(rest) = authority.strip_prefix('[') else {
        return Ok(match authority.split_once(':') {
            None => (authority, None),
            Some((host, text)) => (host, Some(text)),
        });
    };
    let (host, tail) = rest.split_once(']').ok_or(HeadError::BadAuthority)?;
    if tail.is_empty() {
        return Ok((host, None));
    }
    tail.strip_prefix(':')
        .map(|text| (host, Some(text)))
        .ok_or(HeadError::BadAuthority)
}

/// Rewrites the request: origin-form, `Host` from the URI, without hop-by-hop headers,
/// `Connection: close` at the end.
fn rewrite(
    method: &str,
    origin: &str,
    version: &str,
    host: &str,
    headers: &[httparse::Header],
) -> Vec<u8> {
    let mut out = format!("{method} {origin} {version}\r\nHost: {host}\r\n").into_bytes();
    for header in headers.iter().filter(|h| {
        !DROPPED_REQUEST_HEADERS
            .iter()
            .any(|n| h.name.eq_ignore_ascii_case(n))
    }) {
        out.extend_from_slice(header.name.as_bytes());
        out.extend_from_slice(b": ");
        out.extend_from_slice(header.value);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"Connection: close\r\n\r\n");
    out
}

/// Refusal reply: `400`, `502` or `504`, the body is a single line with the reason.
pub(super) fn status_reply(code: u16, body: &str) -> Vec<u8> {
    let phrase = match code {
        400 => "Bad Request",
        504 => "Gateway Timeout",
        _ => "Bad Gateway",
    };
    format!(
        "HTTP/1.1 {code} {phrase}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

#[cfg(test)]
#[path = "http_head_tests.rs"]
mod tests;
