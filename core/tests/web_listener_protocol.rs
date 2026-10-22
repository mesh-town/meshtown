//! Integration tests of the node page listener, the "HTTP protocol over bare
//! TCP" group: body limits, chunked, head buffer overflow, keep-alive/close, an idle
//! connection at shutdown, guard order, HEAD without a body, static files and headers. Three tests
//! and one note are written specifically for today's HTTP stack.
//! What it doesn't do: it doesn't edit `src/web.rs` and `core/web/*.rs` — it only calls their public interface.
//! It never prints the token and never puts it in the text of `assert!`/`assert_eq!`.
//! Links:
//! `docs/WEB-API.md`; scaffolding in `core/tests/common/web_listener.rs` (`#[path]`, shared with
//! `core/tests/web_listener_lifecycle.rs`, `_config_api.rs`, `_network.rs`).

#[path = "common/web_listener.rs"]
mod common;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use meshtown_core::web_gui::DEFAULT_DRAIN;
use serde_json::Value;

use common::{
    cleanup, http, parse_response, raw, read_one_response, run_config, scratch, start_web,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn body_limits_and_bad_json() {
    let dir = scratch("bodylimit");
    let run = run_config(&dir);
    let (handle, _node) = start_web(&dir, &run).await;
    let port = handle.shared.port();
    let token = handle.shared.token();

    // A real body of 262145 bytes (256 KiB + 1) — not a promising header: Limited counts the bytes
    // actually read from the stream.
    let big_body = vec![b'a'; 262_145];
    let resp = http(
        port,
        "PUT",
        "/api/config",
        &[("X-Meshtown-Token", token.as_str())],
        &big_body,
    );
    assert_eq!(resp.status, 413);
    let v: Value = serde_json::from_slice(&resp.body).expect("json");
    assert_eq!(v.get("error").and_then(Value::as_str), Some("too_large"));

    let bad_json_body = b"{not json";
    let resp2 = http(
        port,
        "PUT",
        "/api/config",
        &[("X-Meshtown-Token", token.as_str())],
        bad_json_body,
    );
    assert_eq!(resp2.status, 400);
    let v2: Value = serde_json::from_slice(&resp2.body).expect("json");
    assert_eq!(v2.get("error").and_then(Value::as_str), Some("bad_json"));

    handle.shutdown(DEFAULT_DRAIN).await;
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn chunked_refused() {
    let dir = scratch("chunked");
    let run = run_config(&dir);
    let (handle, _node) = start_web(&dir, &run).await;
    let port = handle.shared.port();
    let token = handle.shared.token();

    // A real chunked request with a valid body inside: the middleware
    // rejects it before the body is read, so the chunk's content doesn't matter.
    let chunk_body = b"{\"a\":1}";
    let request = format!(
        "POST /api/domains HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: http://127.0.0.1:{port}\r\nX-Meshtown-Token: {token}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n",
        chunk_body.len()
    );
    let mut bytes = request.into_bytes();
    bytes.extend_from_slice(chunk_body);
    bytes.extend_from_slice(b"\r\n0\r\n\r\n");
    let raw_bytes = raw(port, &bytes);
    let resp = parse_response(&raw_bytes);
    assert_eq!(resp.status, 400);
    let v: Value = serde_json::from_slice(&resp.body).expect("json");
    assert_eq!(v.get("error").and_then(Value::as_str), Some("bad_request"));
    let conn = resp
        .headers
        .get("connection")
        .map(String::as_str)
        .unwrap_or_default();
    assert_eq!(conn, "close");

    handle.shutdown(DEFAULT_DRAIN).await;
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn head_limits() {
    // 431 is hyper's behaviour at max_buf_size.
    let dir = scratch("headlimit");
    let run = run_config(&dir);
    let (handle, _node) = start_web(&dir, &run).await;
    let port = handle.shared.port();

    let pad = "x".repeat(30_000);
    let request =
        format!("GET /api/state HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nX-Pad: {pad}\r\nConnection: close\r\n\r\n");

    // Its own inline logic over TcpStream (as in keep_alive_and_close), not the shared raw(): we write
    // tolerating a break in the middle of the write (the server may close the connection before the client finishes writing
    // the oversized head — as the shared raw() does), but read to the end of the stream, not to the first
    // pause in reading, so that the "end of stream" below is a verified fact. The pad tail (30 KiB) is larger
    // than the 24 KiB limit, so the server's receive buffer almost always still holds unread
    // bytes when it closes — the kernel answers that with RST, not a clean FIN, so after
    // the 431 the socket is closed with equal probability either by a read of `Ok(0)` or by `ConnectionReset`;
    // both outcomes mean "connection closed", and there must be no third one (data or a timeout).
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_millis(300)))
        .ok();
    let _ = stream.write_all(request.as_bytes());
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    let mut closed = false;
    loop {
        if Instant::now() >= deadline {
            break;
        }
        match stream.read(&mut buf) {
            Ok(0) => {
                closed = true;
                break;
            }
            Ok(n) => out.extend_from_slice(&buf[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => {
                closed = true;
                break;
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(_) => break,
        }
    }

    let resp = parse_response(&out);
    assert_eq!(resp.status, 431, "head buffer overflow must answer 431");
    assert!(
        closed,
        "after the 431 the server must close the connection (end of stream)"
    );

    handle.shutdown(DEFAULT_DRAIN).await;
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn keep_alive_and_close() {
    let dir = scratch("keepalive");
    let run = run_config(&dir);
    let (handle, _node) = start_web(&dir, &run).await;
    let port = handle.shared.port();
    let token = handle.shared.token();

    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();

    let req = format!(
        "GET /api/state HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nX-Meshtown-Token: {token}\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).expect("write 1");
    let resp1 = read_one_response(&mut stream);
    assert_eq!(resp1.status, 200);

    stream.write_all(req.as_bytes()).expect("write 2");
    let resp2 = read_one_response(&mut stream);
    assert_eq!(resp2.status, 200);

    let req3 = format!(
        "GET /api/state HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nX-Meshtown-Token: {token}\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(req3.as_bytes()).expect("write 3");
    let resp3 = read_one_response(&mut stream);
    assert_eq!(resp3.status, 200);
    assert_eq!(
        resp3.headers.get("connection").map(String::as_str),
        Some("close")
    );

    let mut buf = [0u8; 16];
    let n = stream.read(&mut buf).unwrap_or(0);
    assert_eq!(
        n, 0,
        "after Connection: close the server must close the connection"
    );

    // Separately: HTTP/1.0 closes after the response (not checked separately by a probe, we rely on
    // the general compatibility of the hyper server).
    let stream10 = TcpStream::connect(("127.0.0.1", port)).expect("connect 1.0");
    stream10.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let req10 = format!(
        "GET /api/state HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\nX-Meshtown-Token: {token}\r\n\r\n"
    );
    let raw10 = raw(port, req10.as_bytes());
    let _ = stream10; // the connection is used only to send through raw() below
    let resp10 = parse_response(&raw10);
    assert_eq!(resp10.status, 200);

    handle.shutdown(DEFAULT_DRAIN).await;
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_with_idle_connection() {
    let dir = scratch("idleshutdown");
    let run = run_config(&dir);
    let (handle, _node) = start_web(&dir, &run).await;
    let port = handle.shared.port();

    let idle = TcpStream::connect(("127.0.0.1", port)).expect("connect idle");

    let started = Instant::now();
    let ok = handle.shutdown(DEFAULT_DRAIN).await;
    let elapsed = started.elapsed();
    assert!(
        ok,
        "shutdown must return true even with a hanging connection"
    );
    assert!(
        elapsed < Duration::from_millis(2500),
        "shutdown must finish in under 2.5 s, took {elapsed:?}"
    );
    drop(idle);

    let err = TcpStream::connect(("127.0.0.1", port));
    assert!(
        err.is_err(),
        "after the stop the port must not accept connections"
    );

    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guard_order() {
    let dir = scratch("guardorder");
    let run = run_config(&dir);
    let (handle, _node) = start_web(&dir, &run).await;
    let port = handle.shared.port();
    let token = handle.shared.token();

    let resp1 = http(port, "GET", "/api/state", &[("Host", "evil.example")], &[]);
    assert_eq!(resp1.status, 403);
    let v1: Value = serde_json::from_slice(&resp1.body).unwrap();
    assert_eq!(v1.get("error").and_then(Value::as_str), Some("bad_host"));

    // ("Origin", "") is a signal to "not send Origin at all", see the comment at http().
    let resp2 = http(
        port,
        "POST",
        "/api/actions/check-all",
        &[("X-Meshtown-Token", token.as_str()), ("Origin", "")],
        &[],
    );
    assert_eq!(resp2.status, 403);
    let v2: Value = serde_json::from_slice(&resp2.body).unwrap();
    assert_eq!(v2.get("error").and_then(Value::as_str), Some("bad_origin"));

    let resp3 = http(port, "GET", "/api/state", &[], &[]);
    assert_eq!(resp3.status, 401);
    let v3: Value = serde_json::from_slice(&resp3.body).unwrap();
    assert_eq!(v3.get("error").and_then(Value::as_str), Some("bad_token"));

    let resp4 = http(port, "GET", "/css/00-base.css", &[], &[]);
    assert_eq!(resp4.status, 200);

    handle.shutdown(DEFAULT_DRAIN).await;
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn head_has_no_body() {
    let dir = scratch("headnobody");
    let run = run_config(&dir);
    let (handle, _node) = start_web(&dir, &run).await;
    let port = handle.shared.port();
    let token = handle.shared.token();

    let resp = http(
        port,
        "HEAD",
        "/api/state",
        &[("X-Meshtown-Token", token.as_str())],
        &[],
    );
    assert_eq!(resp.status, 200);
    let len: u64 = resp
        .headers
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .expect("content-length must be present");
    assert!(len > 0, "content-length must be greater than zero");
    assert!(resp.body.is_empty(), "the HEAD response body must be empty");

    handle.shutdown(DEFAULT_DRAIN).await;
    cleanup(&dir);
}

/// No caching, no content sniffing and no cross-origin access on a response.
fn assert_hardened_headers(resp: &common::Resp) {
    assert_eq!(
        resp.headers.get("cache-control").map(String::as_str),
        Some("no-store")
    );
    assert_eq!(
        resp.headers
            .get("x-content-type-options")
            .map(String::as_str),
        Some("nosniff")
    );
    assert!(!resp.headers.contains_key("access-control-allow-origin"));
}

/// The root page: HTML, hardened headers and a content policy that allows only the inline icon.
fn assert_html_page(resp: &common::Resp) {
    let ct = resp
        .headers
        .get("content-type")
        .cloned()
        .unwrap_or_default();
    assert!(
        ct.starts_with("text/html"),
        "content-type must start with text/html: {ct}"
    );
    assert_hardened_headers(resp);
    let csp = resp
        .headers
        .get("content-security-policy")
        .cloned()
        .unwrap_or_default();
    assert!(csp.contains("default-src 'none'"), "CSP: {csp}");
    assert!(
        csp.contains("img-src data:"),
        "CSP must allow the data: icon {csp}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn static_and_headers() {
    let dir = scratch("statichdrs");
    let run = run_config(&dir);
    let (handle, _node) = start_web(&dir, &run).await;
    let port = handle.shared.port();
    let token = handle.shared.token();

    let resp_root = http(port, "GET", "/", &[], &[]);
    assert_eq!(resp_root.status, 200);
    assert_html_page(&resp_root);

    let resp_state = http(
        port,
        "GET",
        "/api/state",
        &[("X-Meshtown-Token", token.as_str())],
        &[],
    );
    assert_eq!(resp_state.status, 200);
    assert_hardened_headers(&resp_state);
    assert!(resp_state.headers.contains_key("content-security-policy"));

    let resp_missing = http(port, "GET", "/favicon.ico", &[], &[]);
    assert_eq!(resp_missing.status, 404);

    let resp_bad_method = http(
        port,
        "DELETE",
        "/api/state",
        &[("X-Meshtown-Token", token.as_str())],
        &[],
    );
    assert_eq!(resp_bad_method.status, 405);
    let v: Value = serde_json::from_slice(&resp_bad_method.body).unwrap();
    assert_eq!(v.get("error").and_then(Value::as_str), Some("bad_method"));
    let allow = resp_bad_method
        .headers
        .get("allow")
        .map(String::as_str)
        .unwrap_or_default();
    assert_eq!(allow, "GET,HEAD");

    handle.shutdown(DEFAULT_DRAIN).await;
    cleanup(&dir);
}
