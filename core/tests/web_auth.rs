//! Signing in to the node page: the `/api/auth/*` endpoints through a real listener on loopback.
//! Tokens don't get into `assert!` texts.

#[path = "common/web_auth.rs"]
mod auth_common;
#[path = "common/web_listener.rs"]
mod common;

use std::path::PathBuf;

use meshtown_core::web_gui::{WebHandle, DEFAULT_DRAIN};
use serde_json::json;

use auth_common::*;
use common::{api, cleanup, run_config, scratch, start_web};

struct Rig {
    dir: PathBuf,
    handle: WebHandle,
    port: u16,
    perm: String,
}

async fn rig(tag: &str, password: bool) -> Rig {
    let dir = scratch(tag);
    let run = run_config(&dir);
    if password {
        write_password(&dir, PASSWORD_OK, 1000);
    }
    let (handle, _node) = start_web(&dir, &run).await;
    let port = handle.shared.port();
    let perm = file_token(&dir);
    Rig {
        dir,
        handle,
        port,
        perm,
    }
}

async fn finish(rig: Rig) {
    rig.handle.shutdown(DEFAULT_DRAIN).await;
    cleanup(&rig.dir);
}

fn state_status(rig: &Rig, cookie: &str) -> u16 {
    with_cookie(rig.port, cookie, "GET", "/api/state", None).status
}

macro_rules! rig_test {
    ($name:ident, $password:expr, |$r:ident| $body:block) => {
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn $name() {
            let $r = rig(stringify!($name), $password).await;
            $body
            finish($r).await;
        }
    };
}

rig_test!(temp_token_is_single_use, false, |r| {
    let temp = temp_token(&r.handle.shared.issue_entry_urls()[0]);
    let (first, cookie) = exchange(r.port, &temp);
    assert_eq!(first, 200);
    assert!(cookie.is_some(), "no cookie after the exchange");
    let (second, again) = exchange(r.port, &temp);
    assert_eq!(second, 401);
    assert!(again.is_none());
});

rig_test!(permanent_token_exchange_keeps_header, false, |r| {
    let resp = http(
        r.port,
        "POST",
        "/api/auth/token",
        &[("Content-Type", "application/json")],
        json!({"token": r.perm}).to_string().as_bytes(),
    );
    assert_eq!(resp.status, 200);
    assert_eq!(body_json(&resp)["via"], "token");
    let (status, _) = api(r.port, &r.perm, "GET", "/api/state", None);
    assert_eq!(status, 200, "the header must keep working");
});

rig_test!(foreign_token_string_is_refused, false, |r| {
    let resp = http(
        r.port,
        "POST",
        "/api/auth/token",
        &[("Content-Type", "application/json")],
        json!({"token": "not-a-token-of-this-node"})
            .to_string()
            .as_bytes(),
    );
    assert_eq!(resp.status, 401);
    assert_eq!(body_json(&resp)["error"], "bad_token");
    assert!(
        resp.headers.get("set-cookie").is_none(),
        "no session for a refused token"
    );
});

rig_test!(near_miss_of_permanent_token_is_refused, false, |r| {
    let shorter = &r.perm[..r.perm.len() - 1];
    let longer = format!("{}x", r.perm);
    assert_eq!(exchange(r.port, shorter), (401, None), "a prefix");
    assert_eq!(exchange(r.port, &longer), (401, None), "an extended token");
    assert_eq!(
        exchange(r.port, &r.perm).0,
        200,
        "refusals must not spoil the real token"
    );
});

rig_test!(login_right_and_wrong, true, |r| {
    let (wrong, cookie) = login(r.port, "wrong password");
    assert_eq!(wrong.status, 401);
    assert_eq!(body_json(&wrong)["error"], "bad_password");
    assert!(cookie.is_none());
    let (right, cookie) = login(r.port, PASSWORD_OK);
    assert_eq!(right.status, 200);
    assert_eq!(body_json(&right)["via"], "password");
    assert!(cookie.is_some());
});

rig_test!(sixth_failure_is_429_with_retry_after, true, |r| {
    for _ in 0..5 {
        assert_eq!(login(r.port, "wrong password").0.status, 401);
    }
    let (sixth, cookie) = login(r.port, PASSWORD_OK);
    assert_eq!(
        sixth.status, 429,
        "even the correct password isn't checked during a block"
    );
    assert!(cookie.is_none());
    let wait: u64 = sixth.headers["retry-after"]
        .parse()
        .expect("Retry-After is a number");
    assert!(wait > 0);
});

rig_test!(parallel_failures_count_each, true, |r| {
    let port = r.port;
    let workers: Vec<_> = (0..8)
        .map(|_| std::thread::spawn(move || login(port, "wrong password").0.status))
        .collect();
    let codes: Vec<u16> = workers
        .into_iter()
        .map(|w| w.join().expect("thread"))
        .collect();
    let wrong = codes.iter().filter(|c| **c == 401).count();
    let limited = codes.iter().filter(|c| **c == 429).count();
    assert!(wrong <= 5, "more than five checks: {codes:?}");
    assert_eq!(
        wrong + limited,
        8,
        "the other responses are only 429: {codes:?}"
    );
});

rig_test!(login_without_password_is_409, false, |r| {
    let (resp, cookie) = login(r.port, "whatever password");
    assert_eq!(resp.status, 409);
    assert_eq!(body_json(&resp)["error"], "no_password");
    assert!(cookie.is_none());
});

rig_test!(session_cookie_opens_api_and_logout_closes, false, |r| {
    let (_, cookie) = exchange(r.port, &r.perm);
    let cookie = cookie.expect("cookie");
    assert_eq!(state_status(&r, &cookie), 200);
    let out = with_cookie(r.port, &cookie, "POST", "/api/auth/logout", Some(json!({})));
    assert_eq!(out.status, 200);
    assert!(out.headers["set-cookie"].contains("Max-Age=0"));
    assert_eq!(state_status(&r, &cookie), 401);
});

rig_test!(
    password_change_closes_other_sessions_and_temps,
    false,
    |r| {
        let first = exchange(r.port, &r.perm).1.expect("cookie 1");
        let second = exchange(r.port, &r.perm).1.expect("cookie 2");
        let temp = temp_token(&r.handle.shared.issue_entry_urls()[0]);
        let body = json!({"password": PASSWORD_OK});
        let put = with_cookie(r.port, &first, "PUT", "/api/auth/password", Some(body));
        assert_eq!(put.status, 200);
        assert_eq!(body_json(&put)["password_set"], true);
        assert_eq!(state_status(&r, &first), 200);
        assert_eq!(state_status(&r, &second), 401);
        assert_eq!(exchange(r.port, &temp).0, 401);
        assert_eq!(
            login(r.port, PASSWORD_OK).0.status,
            200,
            "the new password works"
        );
    }
);

rig_test!(reset_requires_header, true, |r| {
    let by_token = exchange(r.port, &r.perm).1.expect("token cookie");
    let by_password = login(r.port, PASSWORD_OK).1.expect("password cookie");
    let denied = with_cookie(r.port, &by_token, "POST", "/api/auth/reset", None);
    assert_eq!(denied.status, 403);
    assert_eq!(body_json(&denied)["error"], "token_only");
    let query = format!("/api/auth/reset?token={}", r.perm);
    assert_eq!(http(r.port, "POST", &query, &[], b"").status, 403);
    assert!(
        r.dir.join("web-password").exists(),
        "the refusal must not touch the file"
    );

    let ok = http(
        r.port,
        "POST",
        "/api/auth/reset",
        &[("X-Meshtown-Token", &r.perm)],
        b"",
    );
    assert_eq!(ok.status, 200);
    let body = body_json(&ok);
    assert!(body["urls"][0]
        .as_str()
        .expect("urls[0]")
        .contains("?token="));
    assert_eq!(body["expires_sec"], 600);
    assert!(!r.dir.join("web-password").exists());
    assert_eq!(state_status(&r, &by_token), 401);
    assert_eq!(state_status(&r, &by_password), 401);
});

rig_test!(rotate_closes_other_sessions, false, |r| {
    let session = exchange(r.port, &r.perm).1.expect("cookie");
    let (status, body) = api(r.port, &r.perm, "POST", "/api/token/rotate", None);
    assert_eq!(status, 200);
    let fresh = body["token"].as_str().expect("new token").to_string();
    assert_eq!(state_status(&r, &session), 401);
    assert_eq!(api(r.port, &fresh, "GET", "/api/state", None).0, 200);
});

fn put_access(r: &Rig, value: &str) -> u16 {
    let (_, got) = api(r.port, &r.perm, "GET", "/api/config", None);
    let rev = got["rev"].as_u64().expect("rev");
    let body = json!({"rev": rev, "config": {"web": {"access": value}}});
    api(r.port, &r.perm, "PUT", "/api/config", Some(body)).0
}

rig_test!(config_put_access_is_live, false, |r| {
    assert_eq!(put_access(&r, "localnet"), 200);
    let status = http(r.port, "GET", "/api/auth/status", &[], b"");
    assert_eq!(body_json(&status)["access"], "localnet");
    assert_eq!(body_json(&status)["listening"], "localhost");
    assert_eq!(put_access(&r, "localhost"), 200);
    let status = http(r.port, "GET", "/api/auth/status", &[], b"");
    assert_eq!(body_json(&status)["access"], "localhost");
    let (_, got) = api(r.port, &r.perm, "GET", "/api/config", None);
    assert_eq!(got["config"]["web"]["access"], "localhost");
});

rig_test!(status_without_credentials, false, |r| {
    let resp = http(r.port, "GET", "/api/auth/status", &[], b"");
    assert_eq!(resp.status, 200);
    let body = body_json(&resp);
    assert!(body["via"].is_null());
    assert_eq!(body["password_set"], false);
});

rig_test!(patch_without_origin_is_403, false, |r| {
    let cookie = exchange(r.port, &r.perm).1.expect("cookie");
    let resp = http(r.port, "PATCH", "/api/config", &[("Cookie", &cookie)], b"");
    assert_eq!(resp.status, 403);
    assert_eq!(body_json(&resp)["error"], "bad_origin");
});

rig_test!(reset_drops_old_temp_tokens, false, |r| {
    let old = temp_token(&r.handle.shared.issue_entry_urls()[0]);
    let ok = http(
        r.port,
        "POST",
        "/api/auth/reset",
        &[("X-Meshtown-Token", &r.perm)],
        b"",
    );
    assert_eq!(ok.status, 200);
    let fresh = temp_token(body_json(&ok)["urls"][0].as_str().expect("urls[0]"));
    assert_eq!(
        exchange(r.port, &old).0,
        401,
        "the old temporary token survived the reset"
    );
    assert_eq!(
        exchange(r.port, &fresh).0,
        200,
        "the new temporary token must work"
    );
});
