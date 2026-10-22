//! Shared scaffolding of the `tests/web_listener_*.rs` scenarios: bringing up the listener (modules 01–04) on port 0
//! (`start_web`) with the fake node `TestNode` (it lives in `test_node.rs` and is re-exported from here), a raw
//! HTTP client (`http`/`api`) and response parsing (`Resp`). Each scenario includes it as
//! `#[path = "common/web_listener.rs"] mod common;`. Each scenario uses only part of this
//! module — the rest is not dead code of the project but functions for a neighbouring file.
#![allow(dead_code)] // each test crate takes its own subset

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use meshtown_core::config::config_file::{self, CliOverrides, ConfigSource, PathRule, RunConfig};
use meshtown_core::web_gui::{self, WebHandle, WebOptions, DEFAULT_DRAIN};
use serde_json::Value;

#[path = "test_node.rs"]
mod test_node;
pub(crate) use test_node::TestNode;

// ============================================================================================
// Scaffolding
// ============================================================================================

/// The test's temporary directory: `$TMPDIR/mesh-web-<tag>-<pid>-<nanos>`, already created.
pub(crate) fn scratch(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    let dir = std::env::temp_dir().join(format!("mesh-web-{tag}-{}-{}", std::process::id(), nanos));
    std::fs::create_dir_all(&dir).expect("scratch: creating the temporary directory");
    dir
}

/// Restores the permissions of the directory and all files in it to `0755`/`0644` (if the test changed them) and removes
/// the directory entirely. Call at the end of every test.
pub(crate) fn cleanup(dir: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755));
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let _ =
                    std::fs::set_permissions(entry.path(), std::fs::Permissions::from_mode(0o644));
            }
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}

/// `config_file::prepare` with an explicit `ConfigSource::File` path in `dir` — as in the page listener's test scaffolding.
pub(crate) fn run_config(dir: &Path) -> RunConfig {
    let mut run = config_file::prepare(
        &ConfigSource::File {
            path: dir.join("config.json"),
            may_create: true,
            rule: PathRule::Explicit,
        },
        &CliOverrides::default(),
        true,
        0,
    )
    .expect("run_config: prepare");
    // Test listeners aren't opened to the local network (the macOS firewall prompt).
    run.effective_settings.process.web_access = config_file::ListenAccess::Localhost;
    run.file_settings.process.web_access = config_file::ListenAccess::Localhost;
    run
}

/// Brings up a listener on port 0 with `TestNode`, the token directory being `dir`.
pub(crate) async fn start_web(dir: &Path, run: &RunConfig) -> (WebHandle, Arc<TestNode>) {
    let node = Arc::new(TestNode::new());
    let opts = WebOptions {
        enabled: true,
        port: 0,
        token_dir: Some(dir.to_path_buf()),
        fixed_token: None,
        drain: DEFAULT_DRAIN,
    };
    let handle = web_gui::start_web_page(opts, run, node.clone())
        .await
        .expect("start_web: web_gui::start must come up");
    (handle, node)
}

/// A parsed response: status, headers in lower case, body.
pub(crate) struct Resp {
    pub(crate) status: u16,
    pub(crate) headers: HashMap<String, String>,
    pub(crate) body: Vec<u8>,
}

/// The index of the byte right after `\r\n\r\n`, if there is one.
fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| i + 4)
}

pub(crate) fn parse_response(raw_bytes: &[u8]) -> Resp {
    let end = find_header_end(raw_bytes).unwrap_or(raw_bytes.len());
    let head = String::from_utf8_lossy(&raw_bytes[..end]);
    let mut lines = head.split("\r\n");
    let status_line = lines.next().unwrap_or("");
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let mut headers = HashMap::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    let body = raw_bytes[end..].to_vec();
    Resp {
        status,
        headers,
        body,
    }
}

/// Opens a `TcpStream`, writes `bytes`, reads to the end of the stream (or to a total timeout of 5 s).
/// Tolerant of write errors: the server may break the connection in the middle of a write.
pub(crate) fn raw(port: u16, bytes: &[u8]) -> Vec<u8> {
    raw_within(port, bytes, Duration::from_secs(5))
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

/// Headers with an empty value ("Host", "") / ("Origin", "") are a signal to "not send this
/// header at all", suppressing the auto-add (used to really omit
/// `Origin`, rather than substitute its value).
pub(crate) fn http(
    port: u16,
    method: &str,
    path: &str,
    extra_headers: &[(&str, &str)],
    body: &[u8],
) -> Resp {
    http_within(
        port,
        method,
        path,
        extra_headers,
        body,
        Duration::from_secs(5),
    )
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

/// A wrapper over `http` for the JSON API: the token in `X-Meshtown-Token`, the body and the response are JSON (an empty response body
/// → `Value::Null`).
pub(crate) fn api(
    port: u16,
    token: &str,
    method: &str,
    path: &str,
    json: Option<Value>,
) -> (u16, Value) {
    let body = json
        .as_ref()
        .map(|v| serde_json::to_vec(v).expect("api: serializing the body"))
        .unwrap_or_default();
    let mut headers: Vec<(&str, &str)> = vec![("X-Meshtown-Token", token)];
    if !body.is_empty() {
        headers.push(("Content-Type", "application/json"));
    }
    let resp = http(port, method, path, &headers, &body);
    let value = if resp.body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&resp.body).unwrap_or(Value::Null)
    };
    (resp.status, value)
}
