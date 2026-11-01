//! Shared scaffolding of the `tests/web_listener_*.rs` scenarios: bringing up the listener (modules 01–04) on port 0
//! (`start_web`) with the fake node `TestNode` (it lives in `test_node.rs` and is re-exported from here), a raw
//! HTTP client (`http_within`) and response parsing (`Resp`). Each scenario includes it as
//! `#[path = "common/web_listener.rs"] mod common;`. What only some scenarios need lives in leaf files next to it
//! (`web_listener_http.rs`, `web_listener_api.rs`, `web_listener_headers.rs`, `web_listener_stream.rs`), which a scenario
//! includes as its own modules and which take this base through `super::common`.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use meshtown_core::config::config_file::{self, CliOverrides, ConfigSource, PathRule, RunConfig};
use meshtown_core::web_gui::{self, WebHandle, WebOptions, DEFAULT_DRAIN};

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
    let handle = web_gui::start_web_page(opts, run, host_of(node.clone(), run))
        .await
        .expect("start_web: web_gui::start must come up");
    (handle, node)
}

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

/// A host with one `main` slot around `node`, with the config state of `run`.
pub(crate) fn host_of(
    node: Arc<dyn web_gui::NodeControl>,
    run: &RunConfig,
) -> Arc<dyn web_gui::HostControl> {
    web_gui::single_slot_host(node, web_gui::ConfigState::from_run(run))
}
