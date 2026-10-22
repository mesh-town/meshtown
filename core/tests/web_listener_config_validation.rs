//! Integration tests of the node page listener, the "config and state" group, the
//! "validation and recovery" part: checking `PUT /api/config` (`invalid`, `not_available`,
//! `read_only`), a directory without write permission, recovery after a broken file, the shape of `/api/state`
//! and `/api/log`. The "network and rev" part is in `web_listener_config_api.rs` (the file went over the 400-line
//! ceiling, so it is split by scenarios).
//! What it doesn't do: it doesn't edit `src/web.rs` and `core/web/*.rs` — it only calls their public interface.
//! It never prints the token and never puts it in the text of `assert!`/`assert_eq!`.
//! Links:
//! `docs/WEB-API.md`; scaffolding in `core/tests/common/web_listener.rs` (`#[path]`, shared with
//! `core/tests/web_listener_config_api.rs`, `_lifecycle.rs`, `_protocol.rs`, `_network.rs`).

#[path = "common/web_listener.rs"]
mod common;

use meshtown_core::config::config_file::{self, LoadedConfigFile, RunConfig};
use meshtown_core::web_gui::{InviteJoinState, InviteJoinView, DEFAULT_DRAIN};
use serde_json::{json, Value};

use common::{api, cleanup, http, run_config, scratch, start_web};

/// `PUT /api/config` with an invalid `socks.port` → 422 `invalid` with the field `socks.port` in `fields`;
/// checks that `rev` didn't change after the refusal.
fn assert_invalid_socks_port_rejected(port: u16, token: &str, rev0: u64, doc: &Value) {
    let mut doc_bad_port = doc.clone();
    doc_bad_port["socks"]["port"] = json!(80);
    let (status1, body1) = api(
        port,
        token,
        "PUT",
        "/api/config",
        Some(json!({"rev": rev0, "config": doc_bad_port})),
    );
    assert_eq!(status1, 422);
    assert_eq!(body1.get("error").and_then(Value::as_str), Some("invalid"));
    let fields1 = body1
        .get("fields")
        .and_then(Value::as_array)
        .expect("fields");
    assert!(
        fields1
            .iter()
            .any(|f| f.get("path").and_then(Value::as_str) == Some("socks.port")),
        "fields must include socks.port"
    );

    let (status_check, cfg_check) = api(port, token, "GET", "/api/config", None);
    assert_eq!(status_check, 200);
    let rev_check = cfg_check.get("rev").and_then(Value::as_u64).expect("rev");
    assert_eq!(rev_check, rev0, "rev must not change after the 422");
}

/// `PUT /api/config` with `autostart` on → 422 `not_available` (the feature is unavailable on this
/// platform/build).
fn assert_autostart_not_available(port: u16, token: &str, rev0: u64, doc: &Value) {
    let mut doc_autostart = doc.clone();
    doc_autostart["autostart"]["enabled"] = json!(true);
    let (status2, body2) = api(
        port,
        token,
        "PUT",
        "/api/config",
        Some(json!({"rev": rev0, "config": doc_autostart})),
    );
    assert_eq!(status2, 422);
    assert_eq!(
        body2.get("error").and_then(Value::as_str),
        Some("not_available")
    );
}

/// `PUT /api/config` with a changed `web.port` → 200, but the field is `read_only`: the response keeps the previous
/// `web.port`, and `changed` is empty.
fn assert_web_port_is_read_only(port: u16, token: &str, rev0: u64, doc: &Value, run: &RunConfig) {
    let mut doc_webport = doc.clone();
    doc_webport["web"]["port"] = json!(9999);
    let (status3, body3) = api(
        port,
        token,
        "PUT",
        "/api/config",
        Some(json!({"rev": rev0, "config": doc_webport})),
    );
    assert_eq!(status3, 200);
    let web_port = body3
        .pointer("/config/web/port")
        .and_then(Value::as_u64)
        .expect("web.port in the response");
    assert_eq!(
        web_port, run.file_settings.process.web_port as u64,
        "web.port must stay as it was (read_only)"
    );
    let changed = body3
        .get("changed")
        .and_then(Value::as_array)
        .expect("changed");
    assert!(
        changed.is_empty(),
        "web.port is read_only — changed must be empty"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn put_validation() {
    let dir = scratch("putvalidation");
    let run = run_config(&dir);
    let (handle, _node) = start_web(&dir, &run).await;
    let port = handle.shared.port();
    let token = handle.shared.token();

    let (status0, cfg0) = api(port, &token, "GET", "/api/config", None);
    assert_eq!(status0, 200);
    let rev0 = cfg0.get("rev").and_then(Value::as_u64).expect("rev0");
    let doc = cfg0.get("config").cloned().unwrap_or(Value::Null);

    assert_invalid_socks_port_rejected(port, &token, rev0, &doc);
    assert_autostart_not_available(port, &token, rev0, &doc);
    assert_web_port_is_read_only(port, &token, rev0, &doc, &run);

    handle.shutdown(DEFAULT_DRAIN).await;
    cleanup(&dir);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn readonly_dir_not_writable() {
    use std::os::unix::fs::PermissionsExt;

    let dir = scratch("readonly");
    let run = run_config(&dir);
    let (handle, _node) = start_web(&dir, &run).await;
    let port = handle.shared.port();
    let token = handle.shared.token();

    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).expect("chmod 555");

    let probe = dir.join(format!("write-probe-{}", std::process::id()));
    let write_worked = std::fs::write(&probe, b"x").is_ok();
    if write_worked {
        let _ = std::fs::remove_file(&probe);
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).ok();
        eprintln!("readonly_dir_not_writable: skipped: running as root");
        handle.shutdown(DEFAULT_DRAIN).await;
        cleanup(&dir);
        return;
    }

    let (status, body) = api(port, &token, "GET", "/api/config", None);
    assert_eq!(status, 200);
    let writable = body
        .pointer("/file/writable")
        .and_then(Value::as_bool)
        .expect("file.writable");
    assert!(!writable, "a 0555 directory must give writable=false");
    let message = body
        .pointer("/file/message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        message.contains("doesn't accept writes"),
        "file.message must explain the reason: {message}"
    );

    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).ok();
    handle.shutdown(DEFAULT_DRAIN).await;
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn put_over_broken_file() {
    let dir = scratch("brokenfile");
    let run = run_config(&dir);
    let (handle, _node) = start_web(&dir, &run).await;
    let port = handle.shared.port();
    let token = handle.shared.token();
    let config_path = dir.join("config.json");

    std::fs::write(&config_path, b"{ broken").expect("corrupting the config file");

    let (status0, cfg0) = api(port, &token, "GET", "/api/config", None);
    assert_eq!(status0, 200);
    let r = cfg0.get("rev").and_then(Value::as_u64).expect("rev");
    let doc = cfg0.get("config").cloned().unwrap_or(Value::Null);

    let (status_put, put_body) = api(
        port,
        &token,
        "PUT",
        "/api/config",
        Some(json!({"rev": r, "config": doc})),
    );
    assert_eq!(status_put, 200);
    let rev_after = put_body
        .get("rev")
        .and_then(Value::as_u64)
        .expect("rev in the response");
    assert_eq!(
        rev_after,
        r + 1,
        "writing from scratch gives 'the last known rev + 1'"
    );

    let has_bad_file = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .any(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("config.json.bad-")
        });
    assert!(has_bad_file, "a config.json.bad-<label> file must appear");

    let loaded = config_file::load_config_file(&config_path);
    assert!(
        matches!(loaded, LoadedConfigFile::Parsed(_)),
        "after the write the file must parse"
    );

    handle.shutdown(DEFAULT_DRAIN).await;
    cleanup(&dir);
}

/// `/api/log` pages: a gap after a cut-off, the line limit and the minimum of one line.
fn assert_log_paging(port: u16, token: &str) {
    let (status_log, log_page) = api(port, token, "GET", "/api/log?after=0&limit=3", None);
    assert_eq!(status_log, 200);
    let lines = log_page
        .get("lines")
        .and_then(Value::as_array)
        .expect("lines");
    assert_eq!(lines.len(), 3);
    let seqs: Vec<u64> = lines
        .iter()
        .filter_map(|l| l.get("seq").and_then(Value::as_u64))
        .collect();
    assert_eq!(seqs, vec![11, 12, 13]);
    assert_eq!(log_page.get("gap").and_then(Value::as_bool), Some(true));

    let (status_log0, log_page0) = api(port, token, "GET", "/api/log?after=0&limit=0", None);
    assert_eq!(status_log0, 200);
    let lines0 = log_page0
        .get("lines")
        .and_then(Value::as_array)
        .expect("lines");
    assert_eq!(lines0.len(), 1, "limit=0 must give 1 line");
}

/// The log download is an attachment named after the node.
fn assert_log_download(port: u16, token: &str) {
    let resp_dl = http(
        port,
        "GET",
        &format!("/api/log/download?token={token}"),
        &[],
        &[],
    );
    assert_eq!(resp_dl.status, 200);
    let cd = resp_dl
        .headers
        .get("content-disposition")
        .cloned()
        .unwrap_or_default();
    assert!(
        cd.contains("meshtown-a1b2c3d4.log"),
        "content-disposition: {cd}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn state_and_log_shape() {
    let dir = scratch("statelogshape");
    let run = run_config(&dir);
    let (handle, _node) = start_web(&dir, &run).await;
    let port = handle.shared.port();
    let token = handle.shared.token();

    let (status, state) = api(port, &token, "GET", "/api/state", None);
    assert_eq!(status, 200);
    let now = state.get("now").and_then(Value::as_f64).expect("now");
    assert!(
        (now - 12.3).abs() < 0.05,
        "now must round to 12.3, got {now}"
    );
    let tail = state
        .pointer("/log/tail")
        .and_then(Value::as_array)
        .expect("log.tail");
    assert_eq!(tail.len(), 6, "log.tail must contain 6 lines");
    for line in tail {
        let t = line.get("t").and_then(Value::as_f64).expect("line's t");
        let rounded = (t * 10.0).round() / 10.0;
        assert!(
            (t - rounded).abs() < 1e-9,
            "a log line's t must be rounded to one decimal place: {t}"
        );
    }
    assert!(
        state.get("config_rev").is_some(),
        "config_rev must be present"
    );
    assert!(
        state
            .pointer("/node/version")
            .and_then(Value::as_str)
            .is_some(),
        "node.version must be present"
    );
    let web_addr = state
        .pointer("/node/web_addr")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert_eq!(web_addr, format!("127.0.0.1:{port}"));
    let identity = state
        .pointer("/node/identity")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert_eq!(identity, "persistent");

    assert_log_paging(port, &token);
    assert_log_download(port, &token);

    handle.shutdown(DEFAULT_DRAIN).await;
    cleanup(&dir);
}

mod invite_join_in_state {
    use super::*;

    mod node_not_waiting_for_an_invite {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn field_is_present_and_null() {
            // when
            let node = node_json_with(None).await;

            // then
            assert_eq!(
                node.get("invite_join"),
                Some(&Value::Null),
                "the key must be there, with a null value"
            );
        }
    }

    mod node_waiting_for_participants {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn field_carries_state_message_label_and_expiry() {
            // when
            let node = node_json_with(Some(view(InviteJoinState::Waiting))).await;

            // then
            assert_eq!(
                node["invite_join"],
                json!({"state": "waiting", "message": "waiting", "label": "Masha", "expires_unix": 1_800_000_000u64})
            );
        }
    }

    mod invite_refused_by_participants {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn state_is_refused() {
            // when
            let node = node_json_with(Some(view(InviteJoinState::Refused))).await;

            // then
            assert_eq!(node["invite_join"]["state"], json!("refused"));
        }
    }

    /// The `node` from the `GET /api/state` response of a listener whose node shows `invite_join`.
    async fn node_json_with(invite_join: Option<InviteJoinView>) -> Value {
        let dir = scratch("invitejoin");
        let run = run_config(&dir);
        let (handle, node) = start_web(&dir, &run).await;
        node.set_invite_join(invite_join);
        let (status, state) = api(
            handle.shared.port(),
            &handle.shared.token(),
            "GET",
            "/api/state",
            None,
        );
        handle.shutdown(DEFAULT_DRAIN).await;
        cleanup(&dir);
        assert_eq!(status, 200);
        state["node"].clone()
    }

    fn view(state: InviteJoinState) -> InviteJoinView {
        InviteJoinView {
            state,
            message: "waiting".to_string(),
            label: Some("Masha".to_string()),
            expires_unix: 1_800_000_000,
        }
    }
}
