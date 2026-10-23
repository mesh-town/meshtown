//! Route table of the node page: every network path answers under both prefixes, every process path once.

use axum::http::{Method, StatusCode};

use super::pages::PAGES;
use crate::web_gui::test_host::send;
use crate::web_gui::test_host::{config_with_key, slot_handle, test_app, FakeHost, SnapNode};

const NET_PATHS: [&str; 12] = [
    "state",
    "config",
    "peer-addresses",
    "network/key",
    "network/invite",
    "network/join",
    "network/create",
    "domains",
    "actions/find-exit",
    "actions/check-domain",
    "actions/check-all",
    "actions/restart",
];

/// The member routes; so far registered under `/api` only.
const MEMBER_PATHS: [&str; 8] = [
    "members",
    "members/exclude",
    "members/unexclude",
    "members/admit",
    "members/unadmit",
    "members/block",
    "members/unblock",
    "members/leave",
];

const PROCESS_PATHS: [&str; 10] = [
    "/api/log",
    "/api/log/download",
    "/api/actions/stop",
    "/api/auth/status",
    "/api/auth/token",
    "/api/auth/login",
    "/api/auth/logout",
    "/api/auth/reset",
    "/api/auth/password",
    "/api/token/rotate",
];

const OTHER: &str = "ab2c3d";

fn app_router() -> axum::Router {
    let main = slot_handle(
        "main",
        SnapNode::new(Default::default()),
        config_with_key(None),
    );
    let other = slot_handle(
        OTHER,
        SnapNode::new(Default::default()),
        config_with_key(None),
    );
    test_app(FakeHost::new(vec![main, other])).0
}

/// A method no route allows: an existing path answers 405 `bad_method`, a missing one 404.
async fn path_exists(router: &axum::Router, path: &str) -> bool {
    let (status, body) = send(router, Method::PATCH, path, "").await;
    status == StatusCode::METHOD_NOT_ALLOWED && body["error"] == "bad_method"
}

#[test]
fn eleven_static_pages() {
    assert_eq!(PAGES.len(), 11, "static pages");
}

#[tokio::test]
async fn net_paths_answer_under_both_prefixes() {
    // given
    let router = app_router();
    let mut found = 0;
    // when
    for base in ["/api", &format!("/api/net/{OTHER}")] {
        for path in NET_PATHS {
            found += usize::from(path_exists(&router, &format!("{base}/{path}")).await);
        }
    }
    for path in MEMBER_PATHS {
        found += usize::from(path_exists(&router, &format!("/api/{path}")).await);
    }
    // then
    assert_eq!(
        found, 32,
        "network paths found under /api and /api/net/<slot>"
    );
}

#[tokio::test]
async fn process_paths_answer_once_and_not_under_a_slot() {
    // given
    let router = app_router();
    let mut found = 0;
    let mut prefixed = 0;
    // when
    for path in PROCESS_PATHS {
        found += usize::from(path_exists(&router, path).await);
        let slotted = path.replacen("/api/", &format!("/api/net/{OTHER}/"), 1);
        prefixed += usize::from(path_exists(&router, &slotted).await);
    }
    // then
    assert_eq!(found, 10, "process paths");
    assert_eq!(prefixed, 0, "process paths under a slot prefix");
}
