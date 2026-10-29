//! `POST /api/auth/link` through a real listener on loopback: header-only, and it changes nothing.
//! Tokens don't get into `assert!` texts.

#[path = "common/web_auth.rs"]
mod auth_common;
#[path = "common/web_listener.rs"]
mod common;

use meshtown_core::web_gui::DEFAULT_DRAIN;

use auth_common::*;
use common::{cleanup, run_config, scratch, start_web};

async fn check(tag: &str, password: bool) {
    let dir = scratch(tag);
    let run = run_config(&dir);
    if password {
        write_password(&dir, PASSWORD_OK, 1000);
    }
    let (handle, _node) = start_web(&dir, &run).await;
    let port = handle.shared.port();
    let perm = file_token(&dir);
    let session = if password {
        login(port, PASSWORD_OK).1
    } else {
        exchange(port, &perm).1
    }
    .expect("session cookie");

    let query = format!("/api/auth/link?token={perm}");
    let denied = http(port, "POST", &query, &[], b"");
    assert_eq!(denied.status, 403);
    assert_eq!(body_json(&denied)["error"], "token_only");
    let by_cookie = with_cookie(port, &session, "POST", "/api/auth/link", None);
    assert_eq!(by_cookie.status, 403);

    let ok = http(
        port,
        "POST",
        "/api/auth/link",
        &[("X-Meshtown-Token", &perm)],
        b"",
    );
    assert_eq!(ok.status, 200);
    let body = body_json(&ok);
    assert_eq!(body["password_set"], password);
    assert_eq!(body["expires_sec"], 600);
    let first = body["urls"][0].as_str().expect("urls[0]");
    assert!(first.contains("127.0.0.1"));
    assert_eq!(first.contains("?token="), !password);

    assert_eq!(dir.join("web-password").exists(), password);
    let state = with_cookie(port, &session, "GET", "/api/state", None);
    assert_eq!(state.status, 200, "the open session must stay");
    handle.shutdown(DEFAULT_DRAIN).await;
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn link_without_password() {
    check("link-nopw", false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn link_with_password() {
    check("link-pw", true).await;
}
