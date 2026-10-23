//! Tests of the `for` note of `GET /api/network/invite`: what reaches the node and what is refused.
//! Driven through the real router with a fake node that remembers the note it was asked to issue with.

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

use crate::web_gui::auth::state::AuthState;
use crate::web_gui::config_api::ConfigState;
use crate::web_gui::control::{ActionError, NodeControl, PendingInvite};
use crate::web_gui::http::ConnInfo;
use crate::web_gui::single_slot_host;
use crate::web_gui::{App, LogPage, NodeSnapshot, WebShared};

const PORT: u16 = 18767;
const TOKEN: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ0123456789ABCDEFGHJK";

/// Remembers the note of every invite it was asked for (`None` — asked without a note).
#[derive(Default)]
struct InviteNode {
    asked: Mutex<Vec<Option<String>>>,
}

impl NodeControl for InviteNode {
    fn snapshot(&self) -> NodeSnapshot {
        NodeSnapshot::default()
    }
    fn log_after(&self, _after: u64, _limit: usize, _net: Option<&str>) -> LogPage {
        LogPage::default()
    }
    fn find_exit(&self, _domain: &str) -> Result<(), ActionError> {
        Ok(())
    }
    fn check_domain(&self, _domain: &str) -> Result<(), ActionError> {
        Ok(())
    }
    fn check_all(&self) -> Result<u64, ActionError> {
        Ok(0)
    }
    fn add_domain(&self, _domain: &str) -> Result<String, ActionError> {
        Ok(String::new())
    }
    fn remove_domain(&self, _domain: &str) -> Result<String, ActionError> {
        Ok(String::new())
    }
    fn add_peer_address(&self, _line: &str) -> Result<(), String> {
        Ok(())
    }
    fn apply_config(
        &self,
        _settings: &crate::config::config_file::Settings,
        _paths: &[&'static str],
    ) {
    }
    fn switch_network(&self, _key: &str) {}
    fn stop(&self) {}

    fn invite_pending(&self, for_whom: Option<&str>) -> Result<PendingInvite, ActionError> {
        self.asked
            .lock()
            .unwrap()
            .push(for_whom.map(str::to_string));
        Ok(PendingInvite::ready("invite-text".to_string()))
    }
}

async fn get(node: &Arc<InviteNode>, path: &str) -> (StatusCode, Value) {
    use crate::config::config_file::{prepare, CliOverrides, ConfigSource, ListenAccess};
    let auth = AuthState::new("mesh_test".to_string(), None, ListenAccess::Localhost);
    let run = prepare(&ConfigSource::Disabled, &CliOverrides::default(), false, 0)
        .expect("prepare without a file");
    let app = Arc::new(App {
        shared: Arc::new(WebShared::new(PORT, TOKEN.to_string(), auth)),
        host: single_slot_host(node.clone(), ConfigState::from_run(&run)),
        token_dir: None,
        stopping: std::sync::atomic::AtomicBool::new(false),
    });
    let host = format!("127.0.0.1:{PORT}");
    let req = Request::builder()
        .method("GET")
        .uri(path)
        .header("host", host.as_str());
    let req = req
        .header("origin", format!("http://{host}"))
        .header("x-meshtown-token", TOKEN);
    let req = req
        .extension(ConnInfo::loopback(PORT))
        .body(Body::empty())
        .unwrap();
    let resp = crate::web_gui::routes::router(app)
        .oneshot(req)
        .await
        .unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn asked(node: &InviteNode) -> Vec<Option<String>> {
    node.asked.lock().unwrap().clone()
}

#[tokio::test]
async fn invite_label_is_passed_to_the_node() {
    // given / when: a note with an encoded space and an apostrophe, padded with plus signs
    let node = Arc::new(InviteNode::default());
    let (status, body) = get(&node, "/api/network/invite?for=%20Anna%27s+laptop+").await;
    // then: the node got the decoded, trimmed note and the reply is the usual one
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["invite"], "invite-text");
    assert_eq!(asked(&node), vec![Some("Anna's laptop".to_string())]);
}

#[tokio::test]
async fn too_long_label_is_invalid() {
    // given / when: 65 characters
    let node = Arc::new(InviteNode::default());
    let (status, body) = get(
        &node,
        &format!("/api/network/invite?for={}", "a".repeat(65)),
    )
    .await;
    // then: 422 invalid and the node was not asked; 64 characters pass
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::UNPROCESSABLE_ENTITY, Some("invalid"))
    );
    assert!(asked(&node).is_empty());
    let (status, _) = get(
        &node,
        &format!("/api/network/invite?for={}", "a".repeat(64)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn control_chars_in_label_are_invalid() {
    // given / when: a line break and a NUL inside the note
    let node = Arc::new(InviteNode::default());
    for raw in ["a%0Ab", "a%00b"] {
        let (status, body) = get(&node, &format!("/api/network/invite?for={raw}")).await;
        // then
        assert_eq!(
            (status, body["error"].as_str()),
            (StatusCode::UNPROCESSABLE_ENTITY, Some("invalid")),
            "{raw}"
        );
    }
    assert!(asked(&node).is_empty());
}

#[tokio::test]
async fn empty_label_means_none() {
    // given / when: no parameter, an empty one and a blank one
    let node = Arc::new(InviteNode::default());
    for path in [
        "/api/network/invite",
        "/api/network/invite?for=",
        "/api/network/invite?for=%20+%20",
    ] {
        let (status, _) = get(&node, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
    }
    // then: the node was always asked without a note
    assert_eq!(asked(&node), vec![None, None, None]);
}
