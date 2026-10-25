//! Integration tests of the gzip-compressed static pages over a live page listener: the content
//! coding headers next to the hardened ones, `HEAD`, a wrong method and the `.gz` path.

#[path = "common/web_listener.rs"]
mod common;

use meshtown_core::web_gui::DEFAULT_DRAIN;
use serde_json::Value;

use common::{cleanup, http, run_config, scratch, start_web};

const GZIP: [(&str, &str); 1] = [("Accept-Encoding", "gzip")];

fn app_js_size() -> u64 {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/web_gui/ui/bundle/app.js");
    std::fs::metadata(path).expect("app.js on disk").len()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn static_page_gzip_keeps_hardened_headers() {
    let dir = scratch("pagegzip");
    let run = run_config(&dir);
    let (handle, _node) = start_web(&dir, &run).await;
    let port = handle.shared.port();

    let resp = http(port, "GET", "/bundle/app.js", &GZIP, &[]);
    assert_eq!(resp.status, 200);
    let h = &resp.headers;
    assert_eq!(h.get("content-encoding").map(String::as_str), Some("gzip"));
    assert_eq!(&resp.body[..2], &[0x1f, 0x8b], "gzip magic");
    assert_eq!(h.get("cache-control").map(String::as_str), Some("no-store"));
    assert_eq!(
        h.get("x-content-type-options").map(String::as_str),
        Some("nosniff")
    );
    let csp = h
        .get("content-security-policy")
        .cloned()
        .unwrap_or_default();
    assert!(csp.contains("default-src 'none'"), "CSP: {csp}");

    handle.shutdown(DEFAULT_DRAIN).await;
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn static_page_head_keeps_gzip_length() {
    let dir = scratch("pagehead");
    let run = run_config(&dir);
    let (handle, _node) = start_web(&dir, &run).await;
    let port = handle.shared.port();

    let resp = http(port, "HEAD", "/bundle/app.js", &GZIP, &[]);
    assert_eq!(resp.status, 200);
    assert_eq!(
        resp.headers.get("content-encoding").map(String::as_str),
        Some("gzip")
    );
    let len: u64 = resp
        .headers
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .expect("content-length must be present");
    assert!(len > 0 && len < app_js_size(), "length {len}");
    assert!(resp.body.is_empty(), "the HEAD response body must be empty");

    handle.shutdown(DEFAULT_DRAIN).await;
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn static_page_wrong_method_and_gz_path() {
    let dir = scratch("pagemethod");
    let run = run_config(&dir);
    let (handle, _node) = start_web(&dir, &run).await;
    let port = handle.shared.port();
    let token = handle.shared.token();

    // PATCH passes the origin and token guards first, then reaches the page route.
    let origin = format!("http://127.0.0.1:{port}");
    let headers = [
        GZIP[0],
        ("Origin", origin.as_str()),
        ("X-Meshtown-Token", token.as_str()),
    ];
    let patched = http(port, "PATCH", "/bundle/app.js", &headers, &[]);
    assert_eq!(patched.status, 405);
    let v: Value = serde_json::from_slice(&patched.body).expect("json");
    assert_eq!(v.get("error").and_then(Value::as_str), Some("bad_method"));
    assert!(!patched.headers.contains_key("content-encoding"));

    let gz_path = http(port, "GET", "/bundle/app.js.gz", &GZIP, &[]);
    assert_eq!(gz_path.status, 404);

    handle.shutdown(DEFAULT_DRAIN).await;
    cleanup(&dir);
}
