//! Integration tests of the node page listener, the "start and stop" group: the token and its
//! rotation, `enabled: false`, a busy port, a stop followed by 503s, a start without a config file.
//! Plus the "stop racing the loop's wake-up" group: `shutdown_true_when_loop_wakes_late`,
//! `shutdown_short_deadline_is_bounded`.
//! The scaffolding works at the socket level — it doesn't depend on a change of the HTTP stack inside the listener.
//! What it doesn't do: it doesn't edit `src/web.rs` and `core/web/*.rs` — it only calls their public interface.
//! It never prints the token and never puts it in the text of `assert!`/`assert_eq!`.
//! Links:
//! `docs/WEB-API.md`; scaffolding in `core/tests/common/web_listener.rs` (`#[path]`, shared with
//! `core/tests/web_listener_protocol.rs`, `_config_api.rs`, `_network.rs`).

#[path = "common/web_listener.rs"]
mod common;

use std::io::Write;
use std::net::TcpStream;
use std::sync::Arc;
use std::time::{Duration, Instant};

use meshtown_core::config::config_file::{self, CliOverrides, ConfigSource};
use meshtown_core::web_gui::{self, WebOptions, WebStartError, DEFAULT_DRAIN, TOKEN_FILE_NAME};
use serde_json::{json, Value};

use common::{api, cleanup, host_of, read_one_response, run_config, scratch, start_web, TestNode};

/// Reads the `web-token` file: permissions 0600 (unix), 52 characters from an alphabet without `i,l,o,u`, matching
/// `handle.shared.token()`. Returns the token from the file.
fn assert_token_file_matches(token_path: &std::path::Path, shared_token: &str) -> String {
    let token_bytes = std::fs::read(token_path).expect("web-token must be readable");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(token_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "web-token permissions must be 0600");
    }
    let file_token = String::from_utf8(token_bytes)
        .expect("the token must be utf8")
        .trim()
        .to_string();
    const ALPHABET: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    assert_eq!(
        file_token.chars().count(),
        52,
        "the token must be 52 characters"
    );
    assert!(
        file_token.chars().all(|c| ALPHABET.contains(c)),
        "the token must be from the alphabet without i,l,o,u"
    );
    assert!(
        file_token == shared_token,
        "the token from the file must match handle.shared.token()"
    );
    file_token
}

/// The token directory must not keep any temporary `.web-token.tmp-*` files.
fn assert_no_tmp_token_left(dir: &std::path::Path) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let name = entry.unwrap().file_name();
        let name = name.to_string_lossy();
        assert!(
            !name.contains(".web-token.tmp-"),
            "no temporary token file must be left: {name}"
        );
    }
}

/// Rotates the token through `/api/token/rotate`, checks the new token on disk and that the old token
/// stopped working while the new one works.
fn rotate_token_and_assert(port2: u16, token2: &str, dir: &std::path::Path) {
    let (status, body) = api(port2, token2, "POST", "/api/token/rotate", None);
    assert_eq!(status, 200);
    let new_token = body
        .get("token")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    assert!(
        new_token != token2,
        "the new token after rotate must differ from the old one"
    );
    let on_disk = std::fs::read_to_string(dir.join(TOKEN_FILE_NAME)).unwrap();
    assert!(
        on_disk.trim() == new_token,
        "the web-token file must be updated with the new token"
    );

    let (status_old, _) = api(port2, token2, "GET", "/api/state", None);
    assert_eq!(status_old, 401, "the old token must stop working");
    let (status_new, _) = api(port2, &new_token, "GET", "/api/state", None);
    assert_eq!(status_new, 200, "the new token must work");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_file_and_rotation() {
    let dir = scratch("token");
    let run = run_config(&dir);
    let (handle, _node) = start_web(&dir, &run).await;

    let token_path = dir.join(TOKEN_FILE_NAME);
    let file_token = assert_token_file_matches(&token_path, &handle.shared.token());
    assert_no_tmp_token_left(&dir);

    let ok = handle.shutdown(DEFAULT_DRAIN).await;
    assert!(ok, "shutdown must meet the deadline");

    let run2 = run_config(&dir);
    let (handle2, _node2) = start_web(&dir, &run2).await;
    let token2 = handle2.shared.token();
    assert!(
        token2 == file_token,
        "a repeated start in the same directory must take the same token"
    );
    let port2 = handle2.shared.port();

    rotate_token_and_assert(port2, &token2, &dir);

    handle2.shutdown(DEFAULT_DRAIN).await;
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disabled_does_not_start() {
    let dir = scratch("disabled");
    let run = run_config(&dir);
    let opts = WebOptions {
        enabled: false,
        port: 0,
        token_dir: Some(dir.clone()),
        fixed_token: None,
        drain: DEFAULT_DRAIN,
    };
    let result =
        web_gui::start_web_page(opts, &run, host_of(Arc::new(TestNode::new()), &run)).await;
    assert!(
        matches!(result, Err(WebStartError::Disabled)),
        "enabled: false must give Err(Disabled)"
    );
    assert!(
        !dir.join(TOKEN_FILE_NAME).exists(),
        "there must be no web-token file"
    );
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn busy_port_is_error() {
    let dir = scratch("busyport");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("occupy the port");
    let busy_port = listener.local_addr().unwrap().port();

    let run = run_config(&dir);
    let opts = WebOptions {
        enabled: true,
        port: busy_port,
        token_dir: Some(dir.clone()),
        fixed_token: None,
        drain: DEFAULT_DRAIN,
    };
    let result =
        web_gui::start_web_page(opts, &run, host_of(Arc::new(TestNode::new()), &run)).await;
    match result {
        Err(WebStartError::Bind { port, .. }) => {
            assert_eq!(port, busy_port, "the error must name the same port");
        }
        Err(other) => panic!("expected WebStartError::Bind, got {other:?}"),
        Ok(_) => panic!("start must not have succeeded on a busy port"),
    }
    drop(listener);
    cleanup(&dir);
}

/// A failed bind leaves the node's token on disk (a second start must recognise the running node by it);
/// a fixed token is not written when the port is busy.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn busy_port_token_file_order() {
    let dir = scratch("busytoken");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("occupy the port");
    let busy_port = listener.local_addr().unwrap().port();
    let run = run_config(&dir);
    let opts = |fixed: Option<String>| WebOptions {
        enabled: true,
        port: busy_port,
        token_dir: Some(dir.clone()),
        fixed_token: fixed,
        drain: DEFAULT_DRAIN,
    };
    let path = dir.join(TOKEN_FILE_NAME);

    let first = web_gui::start_web_page(opts(None), &run, host_of(Arc::new(TestNode::new()), &run));
    assert!(matches!(first.await, Err(WebStartError::Bind { .. })));
    let written = std::fs::read_to_string(&path).expect("token written before the bind");
    let again = web_gui::start_web_page(opts(None), &run, host_of(Arc::new(TestNode::new()), &run));
    assert!(again.await.is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), written);

    let fixed = Some("fixed-token".to_string());
    let third =
        web_gui::start_web_page(opts(fixed), &run, host_of(Arc::new(TestNode::new()), &run));
    assert!(third.await.is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), written);
    drop(listener);
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_then_503() {
    let dir = scratch("stopthen503");
    let run = run_config(&dir);
    let (handle, node) = start_web(&dir, &run).await;
    let port = handle.shared.port();
    let token = handle.shared.token();

    let (status_busy, busy_body) = api(
        port,
        &token,
        "POST",
        "/api/actions/check-domain",
        Some(json!({"domain": "busy.example"})),
    );
    assert_eq!(status_busy, 409);
    assert_eq!(busy_body.get("error").and_then(Value::as_str), Some("busy"));
    let message = busy_body
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(!message.is_empty(), "message must contain the node's text");

    let (status_stop, stop_body) = api(port, &token, "POST", "/api/actions/stop", None);
    assert_eq!(status_stop, 202);
    assert_eq!(
        stop_body.get("stopping").and_then(Value::as_bool),
        Some(true)
    );
    assert!(node.got_call("stop"), "TestNode must get stop");

    let (status0, cfg0) = api(port, &token, "GET", "/api/config", None);
    assert_eq!(status0, 200);
    let rev0 = cfg0.get("rev").and_then(Value::as_u64).expect("rev0");
    let doc = cfg0.get("config").cloned().unwrap_or(Value::Null);
    let (status_put, put_body) = api(
        port,
        &token,
        "PUT",
        "/api/config",
        Some(json!({"rev": rev0, "config": doc})),
    );
    assert_eq!(status_put, 503);
    assert_eq!(
        put_body.get("error").and_then(Value::as_str),
        Some("stopping")
    );

    let (status_state, _) = api(port, &token, "GET", "/api/state", None);
    assert_eq!(status_state, 200);

    handle.shutdown(DEFAULT_DRAIN).await;
    cleanup(&dir);
}

/// Checks `GET /api/config` (`file.path`/`writable`/`message` without a file, `rev == 0`) and
/// `GET /api/state` (`identity: temporary`, `identity_reason: declined`). Returns the body of
/// `/api/config` so the next step can send it back through `PUT`.
fn assert_config_and_state_without_file(port: u16, token: &str) -> Value {
    let (status_cfg, cfg) = api(port, token, "GET", "/api/config", None);
    assert_eq!(status_cfg, 200);
    assert_eq!(cfg.pointer("/file/path"), Some(&Value::Null));
    assert_eq!(
        cfg.pointer("/file/writable").and_then(Value::as_bool),
        Some(false)
    );
    let message = cfg
        .pointer("/file/message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert_eq!(message, "the settings file is not used (--no-config)");
    assert_eq!(cfg.get("rev").and_then(Value::as_u64), Some(0));

    let (status_state, state) = api(port, token, "GET", "/api/state", None);
    assert_eq!(status_state, 200);
    assert_eq!(
        state.pointer("/node/identity").and_then(Value::as_str),
        Some("temporary")
    );
    assert_eq!(
        state
            .pointer("/node/identity_reason")
            .and_then(Value::as_str),
        Some("declined")
    );

    cfg
}

/// Without a config file: `PUT /api/config` → 500, then `/api/network/create` creates a network (200,
/// `switch` on the node), while `rev` in `/api/config` stays 0 and `/api/network/key` returns the same key.
fn assert_create_network_without_file(port: u16, token: &str, concrete: &TestNode, cfg: &Value) {
    let (status_put, _) = api(
        port,
        token,
        "PUT",
        "/api/config",
        Some(json!({"rev": 0, "config": cfg.get("config").cloned().unwrap_or(Value::Null)})),
    );
    assert_eq!(status_put, 500);

    let (status_create, create_body) = api(
        port,
        token,
        "POST",
        "/api/network/create",
        Some(json!({"label": "our gaming room"})),
    );
    assert_eq!(status_create, 200);
    let key = create_body
        .get("key")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    assert!(
        concrete.got_call("switch"),
        "TestNode must get switch after create without a file"
    );

    let (status_rev_after, cfg_after) = api(port, token, "GET", "/api/config", None);
    assert_eq!(status_rev_after, 200);
    assert_eq!(
        cfg_after.get("rev").and_then(Value::as_u64),
        Some(0),
        "rev must stay 0 without a file"
    );

    let (status_key, key_body) = api(port, token, "GET", "/api/network/key", None);
    assert_eq!(status_key, 200);
    assert!(
        key_body.get("key").and_then(Value::as_str) == Some(key.as_str()),
        "GET /api/network/key must return the same key as create"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_config_file() {
    let mut run = config_file::prepare(&ConfigSource::Disabled, &CliOverrides::default(), false, 0)
        .expect("prepare");
    run.effective_settings.process.web_access = config_file::ListenAccess::Localhost;
    let mut opts = WebOptions::for_run(&run);
    assert!(
        opts.token_dir.is_none(),
        "without a config file token_dir must be None"
    );
    opts.port = 0;

    let concrete = Arc::new(TestNode::new());
    let handle = web_gui::start_web_page(opts, &run, host_of(concrete.clone(), &run))
        .await
        .expect("start without a config must come up");
    let port = handle.shared.port();
    let token = handle.shared.token();

    assert_eq!(token.chars().count(), 52, "the token must be 52 characters");
    let cwd_token = std::env::current_dir().unwrap().join(TOKEN_FILE_NAME);
    assert!(
        !cwd_token.exists(),
        "web-token must not appear in the process's current directory"
    );

    let cfg = assert_config_and_state_without_file(port, &token);
    assert_create_network_without_file(port, &token, &concrete, &cfg);

    handle.shutdown(DEFAULT_DRAIN).await;
}

/// Opens a connection to `port`, sends `GET /api/state` with the token and reads one response (200) —
/// the connection stays open (HTTP/1.1 keep-alive), so it is guaranteed to land in the
/// accept loop's `conns`.
fn served_keep_alive_stream(port: u16, token: &str) -> TcpStream {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let req = format!(
        "GET /api/state HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nX-Meshtown-Token: {token}\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).expect("write");
    let resp = read_one_response(&mut stream);
    assert_eq!(resp.status, 200);
    stream
}

/// The race: `deadline`
/// equals `drain`, but the only worker thread is busy with a synchronous `std::thread::sleep`, so
/// the accept loop wakes on the stop signal with a delay δ ≈ 10 ms — noticeably more than the timer
/// tick. The test body runs on the `block_on` thread, not a worker: the outer deadline starts on time,
/// while the inner drain effectively starts δ later.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn shutdown_true_when_loop_wakes_late() {
    let dir = scratch("wakeslate");
    let run = run_config(&dir);
    let opts = WebOptions {
        enabled: true,
        port: 0,
        token_dir: Some(dir.clone()),
        fixed_token: None,
        drain: Duration::from_millis(100),
    };
    let node = Arc::new(TestNode::new());
    let handle = web_gui::start_web_page(opts, &run, host_of(node, &run))
        .await
        .expect("start with drain=100 ms must come up");
    let port = handle.shared.port();
    let token = handle.shared.token();

    let stream = served_keep_alive_stream(port, &token);

    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    meshtown_core::node::output::spawn(async move {
        let _ = started_tx.send(());
        std::thread::sleep(Duration::from_millis(10));
    });
    started_rx
        .await
        .expect("the busy thread must report that it started");

    let began = Instant::now();
    let ok = handle.shutdown(Duration::from_millis(100)).await;
    let elapsed = began.elapsed();

    assert!(
        ok,
        "shutdown must return true: the loop woke late but fit within the tail reserve"
    );
    assert!(
        elapsed < Duration::from_secs(1),
        "shutdown took too long: {elapsed:?}"
    );

    drop(stream);
    cleanup(&dir);
}

/// A neighbouring control: `deadline` shorter than `drain` (100 ms against `DEFAULT_DRAIN`) with a live
/// keep-alive connection — `shutdown` must return `false` and not wait for the whole drain. It is green on the old code
/// too (a guard against overshoot, not a witness of the race).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_short_deadline_is_bounded() {
    let dir = scratch("shortdeadline");
    let run = run_config(&dir);
    let (handle, _node) = start_web(&dir, &run).await;
    let port = handle.shared.port();
    let token = handle.shared.token();

    let stream = served_keep_alive_stream(port, &token);

    let began = Instant::now();
    let ok = handle.shutdown(Duration::from_millis(100)).await;
    let elapsed = began.elapsed();

    assert!(
        !ok,
        "with a short deadline and a live connection shutdown must return false"
    );
    assert!(
        elapsed < Duration::from_secs(1),
        "shutdown must not wait for the whole drain (2 s): took {elapsed:?}"
    );

    drop(stream);
    let err = TcpStream::connect(("127.0.0.1", port));
    assert!(
        err.is_err(),
        "after the stop the port must not accept connections"
    );

    cleanup(&dir);
}
