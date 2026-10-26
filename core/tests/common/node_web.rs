//! Shared scaffolding of the `tests/node_web_*.rs` scenarios: bringing up a real node with the `main.rs` wiring
//! (`boot`/`boot_without_key`), a raw HTTP client (`http`/`api`) and the scenario tail (`teardown`).
//! Each scenario includes it as `#[path = "common/node_web.rs"] mod common;`. Each
//! scenario uses only part of this module — the rest is not dead code of the project but functions for
//! a neighbouring file: `#![allow(dead_code)]` below is about that, not about tolerating real junk.
#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use meshtown_core::config::config_file::NetSlotId;
use meshtown_core::config::config_file::{self, CliOverrides, ConfigSource, PathRule};
use meshtown_core::members::network_key::{managed_key_with_label, NetworkKey};
use meshtown_core::node::host::Host;
use meshtown_core::node::node_ctl::{NodeCtl, Relaunch};
use meshtown_core::node::output;
use meshtown_core::node::{self, IdentityState};
use meshtown_core::proxy::ProxyFront;
use meshtown_core::transport::net::NodeSecret;
use meshtown_core::web_gui::{self, WebHandle, WebOptions, DEFAULT_DRAIN};
use serde_json::{json, Value};
use tokio::time::sleep;

// ============================================================================================
// Scaffolding (in the style of `tests/web_listener_*.rs` — a separate integration-test crate for each
// scenario file, the shared code lives here and is included through `#[path] mod common;`)
// ============================================================================================

/// The test's temporary directory: `$TMPDIR/mesh-node-web-<tag>-<pid>-<nanos>`, already created.
fn scratch(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    let dir = std::env::temp_dir().join(format!(
        "mesh-node-web-{tag}-{}-{}",
        std::process::id(),
        nanos
    ));
    std::fs::create_dir_all(&dir).expect("scratch: creating the temporary directory");
    dir
}

/// Removes the test's temporary directory entirely.
pub(crate) fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

/// Polls the condition every 50 ms until it becomes true or `limit` expires.
pub(crate) async fn wait_until(limit: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + limit;
    loop {
        if cond() {
            return true;
        }
        if Instant::now() >= deadline {
            return cond();
        }
        sleep(Duration::from_millis(50)).await;
    }
}

/// A parsed response: status, headers in lower case, body.
struct Resp {
    status: u16,
    body: Vec<u8>,
}

/// The index of the byte right after `\r\n\r\n`, if there is one.
fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| i + 4)
}

fn parse_response(raw_bytes: &[u8]) -> Resp {
    let end = find_header_end(raw_bytes).unwrap_or(raw_bytes.len());
    let head = String::from_utf8_lossy(&raw_bytes[..end]);
    let status_line = head.split("\r\n").next().unwrap_or("");
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let body = raw_bytes[end..].to_vec();
    Resp { status, body }
}

/// Opens a `TcpStream`, writes `bytes`, reads to the end of the stream (or to a total timeout of 5 s).
fn raw(port: u16, bytes: &[u8]) -> Vec<u8> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("raw: connect");
    stream
        .set_read_timeout(Some(Duration::from_millis(300)))
        .ok();
    let _ = stream.write_all(bytes);
    let deadline = Instant::now() + Duration::from_secs(5);
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

/// Builds and sends one HTTP/1.1 request as raw bytes, parses the response.
fn http(port: u16, method: &str, path: &str, extra_headers: &[(&str, &str)], body: &[u8]) -> Resp {
    let mut req = format!("{method} {path} HTTP/1.1\r\n");
    req.push_str(&format!("Host: 127.0.0.1:{port}\r\n"));
    req.push_str("Connection: close\r\n");
    if !body.is_empty() {
        req.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    if matches!(method, "POST" | "PUT" | "DELETE") {
        req.push_str(&format!("Origin: http://127.0.0.1:{port}\r\n"));
    }
    for (k, v) in extra_headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");

    let mut bytes = req.into_bytes();
    bytes.extend_from_slice(body);
    parse_response(&raw(port, &bytes))
}

/// A wrapper over `http` for the JSON API: the token in `X-Meshtown-Token`, the body and the response are JSON.
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

// ============================================================================================
// Bringing up a real node with the main.rs wiring
// ============================================================================================

/// Handles of the node that was brought up: the temporary directory (`web-token` lives there, the node doesn't print it
/// anywhere except `output::say`, which tests don't look at), the listener, the background task
/// `run_node_with_ctl`, and the page's port and token (taken from `handle.shared`, not from the file — it doesn't matter
/// for the test, but it's shorter).
pub(crate) struct Booted {
    pub(crate) dir: PathBuf,
    pub(crate) handle: WebHandle,
    pub(crate) task: tokio::task::JoinHandle<anyhow::Result<()>>,
    pub(crate) port: u16,
    pub(crate) token: String,
    pub(crate) front: Option<ProxyFront>,
}

/// A node with the `main.rs` wiring and an empty `exit_hosts` table — as on the working path, where it can't be set.
pub(crate) async fn boot(tag: &str, key: &NetworkKey) -> Booted {
    boot_with_exit_hosts(tag, key, HashMap::new()).await
}

/// Like `boot`, but the node is the creator of its Managed network and consults its membership list, as a live
/// node does (`boot` leaves the list open); returns once the node can hand out an invite.
pub(crate) async fn boot_consulting_list(tag: &str, key: &NetworkKey) -> Booted {
    boot_consulting_with_exit_hosts(tag, key, HashMap::new()).await
}

/// `boot_consulting_list` with a table of exit hosts (see `boot_with_exit_hosts`).
pub(crate) async fn boot_consulting_with_exit_hosts(
    tag: &str,
    key: &NetworkKey,
    hosts: HashMap<String, SocketAddr>,
) -> Booted {
    let booted = boot_inner(tag, key, hosts, false).await;
    let can_invite = || {
        api(
            booted.port,
            &booted.token,
            "GET",
            "/api/network/invite",
            None,
        )
        .0 == 200
    };
    assert!(
        wait_until(Duration::from_secs(10), can_invite).await,
        "the creator did not become able to invite within 10 s"
    );
    booted
}

/// The text of the Managed network whose base is `key`, created by the fixed root of the loopback tests:
/// what a person pastes into the key field.
pub(crate) fn key_text(key: &NetworkKey) -> String {
    let root = NodeSecret::from_bytes(&[31; 32]).peer_id();
    managed_key_with_label(None, &key.to_bytes(), &root)
}

/// The hermetic command-line overrides shared by the boots with a network key.
fn hermetic_overrides(key: &NetworkKey) -> CliOverrides {
    CliOverrides {
        network_key: Some(key_text(key)),
        checks_enabled: Some(false),
        socks_port: Some(0),
        web_port: Some(0),
        ..CliOverrides::default()
    }
}

/// Boots a node with the hermetic settings of `hermetic_overrides` (the network key, checks off,
/// `--socks-port 0`, `--web-port 0`); the same `overrides` go both to `prepare` and to `Relaunch.cli`.
/// `hosts` is a test table
/// of exit hosts (`NodeConfig.exit_hosts`): a name → an address bypassing DNS and the address filter; it
/// can't be set from working paths (`main.rs`, config), so only here.
pub(crate) async fn boot_with_exit_hosts(
    tag: &str,
    key: &NetworkKey,
    hosts: HashMap<String, SocketAddr>,
) -> Booted {
    boot_inner(tag, key, hosts, true).await
}

/// The node config of a test boot: no geoip, the given exit hosts, and the creator's secret for loopback networks.
fn test_node_config(
    relaunch: &Relaunch,
    run: &config_file::RunConfig,
    key: &NetworkKey,
    hosts: HashMap<String, SocketAddr>,
    open_list: bool,
) -> node::NodeConfig {
    let mut cfg = relaunch.node_config(
        &run.effective_settings,
        meshtown_core::members::network_key::parse_user_input(&key_text(key))
            .expect("a managed key"),
    );
    cfg.geoip = false; // node tests must not reach out to the internet for geoip sources.
    cfg.exit_hosts = hosts;
    cfg.open_list = open_list;
    if !open_list {
        cfg.identity_secret = Some(NodeSecret::from_bytes(&[31; 32])); // the creator of the loopback networks
    }
    cfg
}

async fn boot_inner(
    tag: &str,
    key: &NetworkKey,
    hosts: HashMap<String, SocketAddr>,
    open_list: bool,
) -> Booted {
    let dir = scratch(tag);
    let overrides = hermetic_overrides(key);
    let source = ConfigSource::File {
        path: dir.join("config.json"),
        may_create: true,
        rule: PathRule::Explicit,
    };
    let mut run = config_file::prepare(&source, &overrides, true, 0).expect("boot: prepare");
    // The test page isn't opened to the local network (the macOS firewall prompt).
    run.effective_settings.process.web_access = config_file::ListenAccess::Localhost;
    output::set_log_capacity(run.effective_settings.process.log_ring_lines as usize);
    let relaunch = Relaunch {
        test_net: Some(meshtown_core::transport::net::mem::process_net()),
        file: run.file.clone(),
        cli: overrides,
        secret: run.secret.clone(),
        initial_domains: Vec::new(),
        blocked_domains: Vec::new(),
    };
    let cfg = test_node_config(&relaunch, &run, key, hosts, open_list);
    let ctl = NodeCtl::new(&run.effective_settings);
    let front = seat_front(&ctl, &run).await;
    let host = host_of(ctl.clone(), &run, front.clone());
    let handle = web_gui::start_web_page(WebOptions::for_run(&run), &run, host)
        .await
        .expect("boot: web_gui::start must come up");
    let port = handle.shared.port();
    let token = handle.shared.token();
    let task = meshtown_core::node::output::spawn(node::run_node_with_ctl(
        cfg,
        run.effective_settings,
        ctl,
        relaunch,
    ));
    Booted {
        dir,
        handle,
        task,
        port,
        token,
        front,
    }
}

/// Like `boot`, but without a network key: `CliOverrides.network_key` stays `None` (the file
/// is created from scratch), the node comes up through `NodeCtl::new_without_network` +
/// `node::run_node_awaiting_key` instead of `NodeCtl::new` + `node::run_node_with_ctl` — it waits for the page's first command
/// instead of bringing up the `Endpoint` immediately.
pub(crate) async fn boot_without_key(tag: &str) -> Booted {
    let dir = scratch(tag);
    let overrides = CliOverrides {
        checks_enabled: Some(false),
        socks_port: Some(0),
        web_port: Some(0),
        ..CliOverrides::default()
    };
    let source = ConfigSource::File {
        path: dir.join("config.json"),
        may_create: true,
        rule: PathRule::Explicit,
    };
    let mut run =
        config_file::prepare(&source, &overrides, true, 0).expect("boot_without_key: prepare");
    run.effective_settings.process.web_access = config_file::ListenAccess::Localhost;
    output::set_log_capacity(run.effective_settings.process.log_ring_lines as usize);
    let relaunch = Relaunch {
        test_net: Some(meshtown_core::transport::net::mem::process_net()),
        file: run.file.clone(),
        cli: overrides,
        secret: run.secret.clone(),
        initial_domains: Vec::new(),
        blocked_domains: Vec::new(),
    };
    let ctl = NodeCtl::new_without_network(&run.effective_settings);
    let front = seat_front(&ctl, &run).await;
    let host = host_of(ctl.clone(), &run, front.clone());
    let handle = web_gui::start_web_page(WebOptions::for_run(&run), &run, host)
        .await
        .expect("boot_without_key: web_gui::start must come up");
    let port = handle.shared.port();
    let token = handle.shared.token();
    let identity = IdentityState::from_report(&run.file_report);
    let task = meshtown_core::node::output::spawn(node::run_node_awaiting_key(
        run.effective_settings,
        identity,
        ctl,
        relaunch,
    ));
    Booted {
        dir,
        handle,
        task,
        port,
        token,
        front,
    }
}

/// Waits until the status leaves `starting` (the snapshot must be built at least once — a 1 s tick),
/// plus a small reserve so that `running_node_for_action()` (`node_ctl.rs:194-205`) doesn't return `Busy` on the first
/// action right after that.
pub(crate) async fn wait_ready(booted: &Booted) {
    let ok = wait_until(Duration::from_secs(10), || {
        let (_, v) = api(booted.port, &booted.token, "GET", "/api/state", None);
        v["node"]["status"] != json!("starting")
    })
    .await;
    assert!(ok, "the status didn't leave starting within 10 s");
    sleep(Duration::from_millis(300)).await;
}

/// Stops the node with a real page action (like scenario (b)), waits for `run_node_with_ctl` no longer than
/// `limit`, shuts the listener down and removes the temporary directory. The common tail of scenarios (a), (c)–(e);
/// (b) checks this same path itself, with a tighter timeout and explicit response codes.
pub(crate) async fn teardown(booted: Booted, limit: Duration) {
    let (status, _) = api(
        booted.port,
        &booted.token,
        "POST",
        "/api/actions/stop",
        None,
    );
    assert_eq!(status, 202, "teardown: stop must return 202");
    let res = tokio::time::timeout(limit, booted.task).await;
    booted.handle.shutdown(DEFAULT_DRAIN).await;
    if let Some(front) = &booted.front {
        front.close().await;
    }
    cleanup(&booted.dir);
    assert!(
        matches!(res, Ok(Ok(Ok(())))),
        "teardown: run_with didn't finish in time or finished with an error"
    );
}

/// A host with one `main` slot around `node`, with the config state of `run` and the process proxy front.
fn host_of(
    node: std::sync::Arc<dyn web_gui::NodeControl>,
    run: &config_file::RunConfig,
    front: Option<ProxyFront>,
) -> std::sync::Arc<dyn web_gui::HostControl> {
    web_gui::single_slot_host_with_front(node, web_gui::ConfigState::from_run(run), front)
}

/// Binds the process proxy front as the host does and seats it on the controller.
async fn seat_front(
    ctl: &std::sync::Arc<NodeCtl>,
    run: &config_file::RunConfig,
) -> Option<ProxyFront> {
    let (front, seat) = Host::seat_proxy(run).await.expect("boot: proxy front");
    ctl.set_proxy_seat(NetSlotId::main(), seat);
    front
}
