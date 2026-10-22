//! Integration tests of the node page listener, the "config and state" group, the
//! "network and rev" part: a network change through `/api/network/create`+`/join`, a stale `rev` after an external
//! write of the config file. The "validation and recovery" part (checking `PUT /api/config`, a directory
//! without write permission, recovery after a broken file, the shape of `/api/state` and `/api/log`) is
//! in `web_listener_config_validation.rs` (the file went over the 400-line ceiling).
//! What it doesn't do: it doesn't edit `src/web.rs` and `core/web/*.rs` — it only calls their public interface.
//! It never prints the token and never puts it in the text of `assert!`/`assert_eq!`: tokens are compared only with
//! `assert!(a == b, "…")` without values in the text (the same rule is kept for the network key).
//! Links:
//! `docs/WEB-API.md`; scaffolding in `core/tests/common/web_listener.rs` (`#[path]`, shared with
//! `core/tests/web_listener_lifecycle.rs`, `_protocol.rs`, `_network.rs`,
//! `_config_validation.rs`).

#[path = "common/web_listener.rs"]
mod common;

use meshtown_core::config::config_file::{self, LoadedConfigFile, RunConfig};
use meshtown_core::members::network_key::parse_user_input;
use meshtown_core::web_gui::DEFAULT_DRAIN;
use serde_json::{json, Value};

use common::{api, cleanup, run_config, scratch, start_web, TestNode};

/// The config file in `dir` holds exactly `key`, in full.
fn assert_file_holds_key(dir: &std::path::Path, key: &str) {
    let loaded = config_file::load_config_file(&dir.join("config.json"));
    match loaded {
        LoadedConfigFile::Parsed(p) => {
            assert!(
                p.settings.net.network_key.as_deref() == Some(key),
                "the config file must hold the just-created key in full"
            );
        }
        other => panic!("expected Loaded::Parsed, got {other:?}"),
    }
}

/// Creates a network through `/api/network/create`, checks the label, the key prefix, the growth of `rev`, the key being written
/// to the config file, the `switch` call on the node, the same key being returned by `/api/network/key`, and
/// no key tail in the body of `GET /api/config`. Returns the key and `rev` after create.
fn assert_network_create_ok(
    port: u16,
    token: &str,
    dir: &std::path::Path,
    rev0: u64,
    node: &TestNode,
) -> (String, u64) {
    let (status, body) = api(
        port,
        token,
        "POST",
        "/api/network/create",
        Some(json!({"label": "our gaming room"})),
    );
    assert_eq!(status, 200);
    assert_eq!(
        body.get("label").and_then(Value::as_str),
        Some("our-gaming-room")
    );
    let key = body
        .get("key")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    assert!(
        key.starts_with("our-gaming-room--"),
        "the key must start with the label"
    );

    let (status1, cfg1) = api(port, token, "GET", "/api/config", None);
    assert_eq!(status1, 200);
    let rev1 = cfg1.get("rev").and_then(Value::as_u64).expect("rev1");
    assert_eq!(rev1, rev0 + 1, "rev must grow by 1 after create");

    assert_file_holds_key(dir, &key);

    assert!(
        node.got_call("switch"),
        "TestNode must get switch after create"
    );

    let (status_key, key_body) = api(port, token, "GET", "/api/network/key", None);
    assert_eq!(status_key, 200);
    assert!(
        key_body.get("key").and_then(Value::as_str) == Some(key.as_str()),
        "GET /api/network/key must return the same key as create"
    );

    let cfg1_text = serde_json::to_string(&cfg1).unwrap();
    let tail_part = key
        .split_once("--")
        .map(|x| x.1)
        .expect("key tail after --");
    assert!(
        !cfg1_text.contains(tail_part),
        "the body of GET /api/config must not contain the key tail after --"
    );

    (key, rev1)
}

/// Joins the just-created network by its key (202, `rev` grows by 1), then checks
/// that garbage in place of a key doesn't parse and that `join` with it answers 422 `invalid_key` without the field
/// `word`.
fn assert_join_valid_then_rejects_garbage(port: u16, token: &str, key: &str, rev1: u64) {
    let (status_join, _join_body) = api(
        port,
        token,
        "POST",
        "/api/network/join",
        Some(json!({"key": key})),
    );
    assert_eq!(status_join, 202);
    let (status2, cfg2) = api(port, token, "GET", "/api/config", None);
    assert_eq!(status2, 200);
    let rev2 = cfg2.get("rev").and_then(Value::as_u64).expect("rev2");
    assert_eq!(rev2, rev1 + 1, "rev must grow by 1 after join");

    assert!(
        parse_user_input("not a key").is_err(),
        "precondition: 'not a key' must not parse"
    );
    let (status_bad, bad_body) = api(
        port,
        token,
        "POST",
        "/api/network/join",
        Some(json!({"key": "not a key"})),
    );
    assert_eq!(status_bad, 422);
    assert_eq!(
        bad_body.get("error").and_then(Value::as_str),
        Some("invalid_key")
    );
    assert!(
        bad_body.get("word").is_none(),
        "there must be no word field"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn network_create_and_join() {
    let dir = scratch("netcreate");
    let run = run_config(&dir);
    let (handle, node) = start_web(&dir, &run).await;
    let port = handle.shared.port();
    let token = handle.shared.token();

    let (status0, cfg0) = api(port, &token, "GET", "/api/config", None);
    assert_eq!(status0, 200);
    let rev0 = cfg0.get("rev").and_then(Value::as_u64).expect("rev0");

    let (key, rev1) = assert_network_create_ok(port, &token, &dir, rev0, &node);
    assert_join_valid_then_rejects_garbage(port, &token, &key, rev1);

    handle.shutdown(DEFAULT_DRAIN).await;
    cleanup(&dir);
}

/// Edits the config file bypassing the web listener (the same `rev` the client already saw), then sends `PUT`
/// with the old `rev` and checks for 409 `stale_rev` with a grown `rev` and the external value pulled in.
/// Returns the current `rev` from before the external write and the body of the 409 response (useful for a repeated
/// `PUT`).
fn save_outside_and_assert_stale(
    port: u16,
    token: &str,
    config_path: &std::path::Path,
    run: &RunConfig,
    cfg0: &Value,
) -> (u64, Value) {
    let r = cfg0.get("rev").and_then(Value::as_u64).expect("rev");

    let loaded = config_file::load_config_file(config_path);
    let mut settings = match loaded {
        LoadedConfigFile::Parsed(p) => p.settings.clone(),
        other => panic!("expected Loaded::Parsed, got {other:?}"),
    };
    settings.net.check_interval_sec = 900;
    config_file::save_config_file(config_path, Some(r), 0, &run.secret, &settings)
        .expect("external save");

    let (status_stale, stale_body) = api(
        port,
        token,
        "PUT",
        "/api/config",
        Some(json!({"rev": r, "config": cfg0.get("config").cloned().unwrap_or(Value::Null)})),
    );
    assert_eq!(status_stale, 409);
    assert_eq!(
        stale_body.get("error").and_then(Value::as_str),
        Some("stale_rev")
    );
    let rev_after = stale_body
        .get("rev")
        .and_then(Value::as_u64)
        .expect("rev in the 409");
    assert_eq!(rev_after, r + 1);
    let interval = stale_body
        .pointer("/config/checks/interval_sec")
        .and_then(Value::as_u64)
        .expect("checks.interval_sec in the 409 response");
    assert_eq!(interval, 900);

    (r, stale_body)
}

/// Repeats `PUT /api/config` with the pulled-in `rev` (must succeed), then checks the file permissions
/// 0600 (unix) and that no temporary `.tmp` files are left in the working directory.
#[cfg_attr(not(unix), allow(unused_variables))] // only the unix branch uses the paths
fn assert_retry_succeeds_and_no_tmp_left(
    port: u16,
    token: &str,
    config_path: &std::path::Path,
    dir: &std::path::Path,
    r: u64,
    stale_body: &Value,
) {
    let (status_ok, ok_body) = api(
        port,
        token,
        "PUT",
        "/api/config",
        Some(
            json!({"rev": r + 1, "config": stale_body.get("config").cloned().unwrap_or(Value::Null)}),
        ),
    );
    assert_eq!(status_ok, 200);
    let _ = ok_body;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(config_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "config.json must have permissions 0600 after writing"
        );
    }
    for entry in std::fs::read_dir(dir).unwrap() {
        let name = entry.unwrap().file_name();
        let name = name.to_string_lossy();
        assert!(
            !name.contains(".tmp"),
            "no .tmp files must be left in the temporary directory: {name}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stale_rev_after_outside_save() {
    let dir = scratch("stale");
    let run = run_config(&dir);
    let (handle, _node) = start_web(&dir, &run).await;
    let port = handle.shared.port();
    let token = handle.shared.token();
    let config_path = dir.join("config.json");

    let (status0, cfg0) = api(port, &token, "GET", "/api/config", None);
    assert_eq!(status0, 200);

    let (r, stale_body) = save_outside_and_assert_stale(port, &token, &config_path, &run, &cfg0);
    assert_retry_succeeds_and_no_tmp_left(port, &token, &config_path, &dir, r, &stale_body);

    handle.shutdown(DEFAULT_DRAIN).await;
    cleanup(&dir);
}
