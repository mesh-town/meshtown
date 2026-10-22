//! Integration tests of the node page listener: `POST /api/actions/restart` (the "Restart
//! node" button) — the `NodeControl::restart` call, the shared router layers (Origin, token, method) and 503
//! after a stop. The scaffolding is `core/tests/common/web_listener.rs` (`#[path]`, shared with the other
//! `tests/web_listener_*.rs`). What it doesn't do: it doesn't edit `core/web/*.rs` — it only calls their
//! public interface. It never prints the token.

#[path = "common/web_listener.rs"]
mod common;

use meshtown_core::web_gui::DEFAULT_DRAIN;
use serde_json::{json, Value};

use common::{api, cleanup, http, run_config, scratch, start_web};

const RESTART: &str = "/api/actions/restart";

mod post_restart {
    use super::*;

    mod with_token_and_origin {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn is_accepted_and_restart_called() {
            let s = setup().await;

            let (status, body) = api(s.port, &s.token, "POST", RESTART, None);

            assert_eq!(status, 202);
            assert_eq!(body["restart"], json!(true));
            assert!(s.node.got_call("restart"), "TestNode must get restart");
            s.teardown().await;
        }
    }

    mod without_origin {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn is_forbidden_and_not_called() {
            let s = setup().await;

            // ("Origin", "") is a signal to "not send Origin at all", see `http()`.
            let resp = http(
                s.port,
                "POST",
                RESTART,
                &[("X-Meshtown-Token", s.token.as_str()), ("Origin", "")],
                &[],
            );

            assert_eq!(resp.status, 403);
            let v: Value = serde_json::from_slice(&resp.body).unwrap();
            assert_eq!(v["error"], json!("bad_origin"));
            assert!(!s.node.got_call("restart"), "restart must not be called");
            s.teardown().await;
        }
    }

    mod without_token {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn is_unauthorized_and_not_called() {
            let s = setup().await;

            let resp = http(s.port, "POST", RESTART, &[], &[]);

            assert_eq!(resp.status, 401);
            let v: Value = serde_json::from_slice(&resp.body).unwrap();
            assert_eq!(v["error"], json!("bad_token"));
            assert!(!s.node.got_call("restart"), "restart must not be called");
            s.teardown().await;
        }
    }

    mod get_method {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn is_bad_method() {
            let s = setup().await;

            let (status, body) = api(s.port, &s.token, "GET", RESTART, None);

            assert_eq!(status, 405);
            assert_eq!(body["error"], json!("bad_method"));
            assert!(!s.node.got_call("restart"), "restart must not be called");
            s.teardown().await;
        }
    }

    mod after_stop {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn is_stopping_and_not_called() {
            let s = setup().await;
            let (stop_status, _) = api(s.port, &s.token, "POST", "/api/actions/stop", None);
            assert_eq!(stop_status, 202, "the stop must succeed");

            let (status, body) = api(s.port, &s.token, "POST", RESTART, None);

            assert_eq!(status, 503);
            assert_eq!(body["error"], json!("stopping"));
            assert!(
                !s.node.got_call("restart"),
                "restart is not called after stop"
            );
            s.teardown().await;
        }
    }
}

struct Fixture {
    dir: std::path::PathBuf,
    handle: meshtown_core::web_gui::WebHandle,
    node: std::sync::Arc<common::TestNode>,
    port: u16,
    token: String,
}

impl Fixture {
    async fn teardown(self) {
        self.handle.shutdown(DEFAULT_DRAIN).await;
        cleanup(&self.dir);
    }
}

async fn setup() -> Fixture {
    let dir = scratch("restart");
    let run = run_config(&dir);
    let (handle, node) = start_web(&dir, &run).await;
    let port = handle.shared.port();
    let token = handle.shared.token();
    Fixture {
        dir,
        handle,
        node,
        port,
        token,
    }
}
