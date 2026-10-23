//! Minimal HTTP/1.1 client over a ready stream, plus the shared TLS setup for checks.
//!
//! What it does: builds the TLS client config (ring provider, webpki roots, ALPN only
//! http/1.1); sends one GET request into an already open stream (TCP or TLS), reads the response with
//! a size cap and a per-read timeout, counts the bytes that arrived over the connection
//! and flags a stalled read — a sign of throttling by a national DPI filter.
//! What it does not do: it does not open connections, resolve names or keep a connection pool,
//! and does not support HTTP/2, redirects, compression or request retries.
//! Links: used by `crate::site_checks::doh` (DoH request) and `crate::site_checks::reachability` (domain check).

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use rustls::{ClientConfig, RootCertStore};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::timeout;

/// Limits of a single request.
#[derive(Debug, Clone, Copy)]
pub struct HttpLimits {
    /// Timeout for connection setup and the TLS handshake.
    pub connect: Duration,
    /// Timeout for a single read from the stream; exceeding it counts as a stall.
    pub read: Duration,
    /// Total time limit for reading the response.
    pub total: Duration,
    /// Maximum number of bytes to read, headers included.
    pub max_bytes: usize,
}

impl Default for HttpLimits {
    fn default() -> Self {
        HttpLimits {
            connect: Duration::from_secs(8),
            read: Duration::from_secs(12),
            total: Duration::from_secs(30),
            max_bytes: 64 * 1024,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HttpGetRequest<'a> {
    pub host: &'a str,
    pub path: &'a str,
    pub accept: &'a str,
    pub user_agent: &'a str,
}

/// Response. `status` is zero if the headers did not arrive in full.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// Total bytes received over the connection, headers included.
    pub raw_bytes: usize,
    /// The read stalled: no byte within `HttpLimits::read`, or the total time ran out.
    pub stalled: bool,
    /// The server closed the connection itself.
    pub eof: bool,
}

impl HttpResponse {
    /// Returns the header value by name, case-insensitively.
    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| v.as_str())
    }
}

/// TLS client config for checks: `ring` provider, `webpki-roots` roots, http/1.1 only.
/// The provider is passed explicitly and the process-wide provider is left alone.
pub fn tls_client_config() -> Result<ClientConfig> {
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut cfg = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .context("TLS version set not supported by the ring provider")?
        .with_root_certificates(roots)
        .with_no_client_auth();
    cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(cfg)
}

/// Sends GET and reads the response. An error is returned only if the stream failed before the first
/// response byte; everything else is `HttpResponse` fields, on which `crate::site_checks::reachability` decides.
pub async fn http_get<S>(
    stream: &mut S,
    req: HttpGetRequest<'_>,
    limits: HttpLimits,
) -> Result<HttpResponse>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let head = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: {}\r\nAccept: {}\r\nAccept-Encoding: identity\r\nConnection: close\r\n\r\n",
        req.path, req.host, req.user_agent, req.accept
    );
    timeout(limits.read, stream.write_all(head.as_bytes()))
        .await
        .context("request send timeout")?
        .context("request not sent")?;
    timeout(limits.read, stream.flush())
        .await
        .context("request send timeout")?
        .context("request not sent")?;

    let deadline = Instant::now() + limits.total;
    let mut raw: Vec<u8> = Vec::with_capacity(8 * 1024);
    let mut buf = [0u8; 4096];
    let mut stalled = false;
    let mut eof = false;
    loop {
        if raw.len() >= limits.max_bytes || complete(&raw) {
            break;
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            stalled = true;
            break;
        }
        match timeout(limits.read.min(left), stream.read(&mut buf)).await {
            Err(_) => {
                stalled = true;
                break;
            }
            Ok(Ok(0)) => {
                eof = true;
                break;
            }
            Ok(Ok(n)) => raw.extend_from_slice(&buf[..n]),
            Ok(Err(e)) => {
                if raw.is_empty() {
                    return Err(e).context("reading the response");
                }
                eof = true;
                break;
            }
        }
    }
    Ok(parse(&raw, stalled, eof))
}

/// Returns whether the response has arrived in full: by `Content-Length` or by the last `chunked` chunk.
fn complete(raw: &[u8]) -> bool {
    let Some(head_end) = find(raw, b"\r\n\r\n") else {
        return false;
    };
    let body = &raw[head_end + 4..];
    let head = String::from_utf8_lossy(&raw[..head_end]).to_ascii_lowercase();
    if head.contains("transfer-encoding: chunked") {
        return body.starts_with(b"0\r\n\r\n") || find(body, b"\r\n0\r\n\r\n").is_some();
    }
    for line in head.split("\r\n") {
        if let Some(v) = line.strip_prefix("content-length:") {
            if let Ok(len) = v.trim().parse::<usize>() {
                return body.len() >= len;
            }
        }
    }
    false
}

/// Parses whatever has arrived. Incomplete headers mean `status` 0, not an error.
fn parse(raw: &[u8], stalled: bool, eof: bool) -> HttpResponse {
    let mut resp = HttpResponse {
        status: 0,
        headers: Vec::new(),
        body: Vec::new(),
        raw_bytes: raw.len(),
        stalled,
        eof,
    };
    let Some(head_end) = find(raw, b"\r\n\r\n") else {
        return resp;
    };
    let head = String::from_utf8_lossy(&raw[..head_end]).to_string();
    let mut lines = head.split("\r\n");
    if let Some(first) = lines.next() {
        if let Some(code) = first.split_whitespace().nth(1) {
            resp.status = code.parse().unwrap_or(0);
        }
    }
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            resp.headers
                .push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
        }
    }
    let chunked = resp
        .header("transfer-encoding")
        .map(|v| v.to_ascii_lowercase().contains("chunked"))
        .unwrap_or(false);
    let content_len = resp
        .header("content-length")
        .and_then(|v| v.trim().parse::<usize>().ok());
    let body = &raw[head_end + 4..];
    resp.body = if chunked {
        decode_chunked(body)
    } else if let Some(len) = content_len {
        body[..len.min(body.len())].to_vec()
    } else {
        body.to_vec()
    };
    resp
}

/// Joins `chunked` chunks; stops silently if the body did not arrive in full.
fn decode_chunked(mut data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    while let Some(pos) = find(data, b"\r\n") {
        let line = &data[..pos];
        let head = line.split(|b| *b == b';').next().unwrap_or(line);
        let Ok(text) = std::str::from_utf8(head) else {
            break;
        };
        let Ok(size) = usize::from_str_radix(text.trim(), 16) else {
            break;
        };
        if size == 0 {
            break;
        }
        let start = pos + 2;
        let end = start + size;
        if end > data.len() {
            out.extend_from_slice(&data[start..]);
            break;
        }
        out.extend_from_slice(&data[start..end]);
        if end + 2 > data.len() {
            break;
        }
        data = &data[end + 2..];
    }
    out
}

/// Position of the first occurrence of a substring, in bytes.
fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
#[path = "http_min_tests.rs"]
mod tests;
