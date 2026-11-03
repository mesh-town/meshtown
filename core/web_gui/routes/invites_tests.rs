//! Tests of the open-invite routes (`invites.rs`): the list and the note edit, driven through the real router
//! with a fake node that answers by a given outcome and remembers the edit it was asked for.

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::web_gui::auth::state::AuthState;
use crate::web_gui::config_api::ConfigState;
use crate::web_gui::http::ConnInfo;
use crate::web_gui::{
    single_slot_host, ActionError, App, InviteRow, InvitesFuture, InvitesReply, LogPage,
    MemberError, NodeControl, NodeSnapshot, WebShared,
};

const PORT: u16 = 18767;
const TOKEN: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ0123456789ABCDEFGHJK";
const HASH: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

/// What the fake node answers.
enum Answer {
    List(InvitesReply),
    Refused(&'static str),
    Action(ActionError),
}

/// Edits the node was asked for: hash and the note.
type Seen = Arc<Mutex<Vec<(String, Option<String>)>>>;

struct FakeNode {
    answer: Answer,
    seen: Seen,
}

impl FakeNode {
    fn reply(&self) -> InvitesFuture {
        let result = match &self.answer {
            Answer::List(reply) => Ok(reply.clone()),
            Answer::Refused(code) => Err(MemberError::Refused {
                code,
                message: "refused".to_string(),
            }),
            Answer::Action(err) => Err(MemberError::Action(err.clone())),
        };
        Box::pin(async move { result })
    }
}

impl NodeControl for FakeNode {
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
    fn apply_config(&self, _s: &crate::config::config_file::Settings, _p: &[&'static str]) {}
    fn switch_network(&self, _key: &str) {}
    fn stop(&self) {}

    fn open_invites(&self) -> InvitesFuture {
        self.reply()
    }

    fn relabel_invite(&self, hash: String, for_whom: Option<String>) -> InvitesFuture {
        self.seen.lock().unwrap().push((hash, for_whom));
        self.reply()
    }
}

struct Fixture {
    router: Router,
    seen: Seen,
}

fn sample() -> InvitesReply {
    InvitesReply {
        invites: vec![InviteRow {
            hash: HASH.to_string(),
            for_whom: Some("Anna".to_string()),
            made_unix: 1_000,
            expires_unix: 2_000,
            issuer: Some("ab".repeat(32)),
            issuer_name: Some("laptop".to_string()),
            mine: true,
        }],
    }
}

fn setup(answer: Answer) -> Fixture {
    use crate::config::config_file::{prepare, CliOverrides, ConfigSource, ListenAccess};
    let seen = Seen::default();
    let node = Arc::new(FakeNode {
        answer,
        seen: seen.clone(),
    });
    let auth = AuthState::new("mesh_test".to_string(), None, ListenAccess::Localhost);
    let run = prepare(&ConfigSource::Disabled, &CliOverrides::default(), false, 0).unwrap();
    let app = Arc::new(App {
        shared: Arc::new(WebShared::new(PORT, TOKEN.to_string(), auth)),
        host: single_slot_host(node, ConfigState::from_run(&run)),
        token_dir: None,
        stopping: std::sync::atomic::AtomicBool::new(false),
        pending_invites: Default::default(),
    });
    Fixture {
        router: crate::web_gui::routes::router(app),
        seen,
    }
}

async fn send(fx: &Fixture, method: &str, path: &str, body: &str) -> (StatusCode, Value) {
    let host = format!("127.0.0.1:{PORT}");
    let req = Request::builder()
        .method(method)
        .uri(path)
        .header("host", host.as_str())
        .header("origin", format!("http://{host}"))
        .header("x-meshtown-token", TOKEN)
        .extension(ConnInfo::loopback(PORT))
        .body(Body::from(body.to_string()))
        .unwrap();
    let resp = fx.router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn label(fx: &Fixture, body: Value) -> (StatusCode, Value) {
    send(fx, "POST", "/api/network/invites/label", &body.to_string()).await
}

fn line((status, body): (StatusCode, Value)) -> String {
    let code = body["error"].as_str().unwrap_or("-");
    format!("{} {code}", status.as_u16())
}

#[tokio::test]
async fn get_returns_the_list() {
    // given
    let fx = setup(Answer::List(sample()));
    // when
    let (status, body) = send(&fx, "GET", "/api/network/invites", "").await;
    // then
    assert_eq!(status, StatusCode::OK);
    let row = &body["invites"][0];
    assert_eq!(row["hash"], HASH);
    assert_eq!(row["for_whom"], "Anna");
    assert_eq!(row["expires_unix"], 2_000);
    assert_eq!(row["issuer_name"], "laptop");
    assert_eq!(row["mine"], true);
}

#[tokio::test]
async fn post_label_passes_hash_and_trimmed_label() {
    // given
    let fx = setup(Answer::List(sample()));
    // when
    let (status, body) = label(&fx, json!({"hash": HASH, "for": "  Ann "})).await;
    // then
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["invites"][0]["hash"], HASH);
    let seen = fx.seen.lock().unwrap().clone();
    assert_eq!(seen, [(HASH.to_string(), Some("Ann".to_string()))]);
}

#[tokio::test]
async fn blank_label_means_none() {
    // given
    let fx = setup(Answer::List(sample()));
    // when
    label(&fx, json!({"hash": HASH, "for": "   "})).await;
    label(&fx, json!({"hash": HASH})).await;
    // then
    let seen = fx.seen.lock().unwrap().clone();
    assert_eq!(seen[0].1, None);
    assert_eq!(seen[1].1, None);
}

#[tokio::test]
async fn non_string_label_is_400() {
    // given
    let fx = setup(Answer::List(sample()));
    // when
    let got = label(&fx, json!({"hash": HASH, "for": 5})).await;
    // then
    assert_eq!(line(got), "400 bad_request");
    assert!(fx.seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn uppercase_hash_reaches_the_node_lowercase() {
    // given
    let fx = setup(Answer::List(sample()));
    // when
    label(&fx, json!({"hash": HASH.to_uppercase(), "for": "x"})).await;
    // then
    assert_eq!(fx.seen.lock().unwrap()[0].0, HASH);
}

#[tokio::test]
async fn long_label_is_422() {
    // given
    let fx = setup(Answer::List(sample()));
    // when
    let got = label(&fx, json!({"hash": HASH, "for": "a".repeat(65)})).await;
    // then
    assert_eq!(line(got), "422 invalid");
    assert!(fx.seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn control_chars_label_is_422() {
    // given
    let fx = setup(Answer::List(sample()));
    // when / then
    for bad in ["a\nb", "a\u{2028}b"] {
        let got = label(&fx, json!({"hash": HASH, "for": bad})).await;
        assert_eq!(line(got), "422 invalid", "{bad:?}");
    }
    assert!(fx.seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn bad_hash_is_400() {
    // given
    let fx = setup(Answer::List(sample()));
    // when / then: no hash, a short one, a non-hex one, not an object
    for body in [
        json!({}),
        json!({"hash": "abc"}),
        json!({"hash": "z".repeat(64)}),
    ] {
        assert_eq!(line(label(&fx, body).await), "400 bad_request");
    }
    let raw = send(&fx, "POST", "/api/network/invites/label", "[]").await;
    assert_eq!(line(raw), "400 bad_request");
    assert!(fx.seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn not_admin_is_409() {
    // given
    let fx = setup(Answer::Refused("not_admin"));
    // when
    let got = send(&fx, "GET", "/api/network/invites", "").await;
    // then
    assert_eq!(line(got), "409 not_admin");
}

#[tokio::test]
async fn not_issuer_is_409() {
    // given
    let fx = setup(Answer::Refused("not_issuer"));
    // when
    let got = label(&fx, json!({"hash": HASH, "for": "x"})).await;
    // then
    assert_eq!(line(got), "409 not_issuer");
}

#[tokio::test]
async fn not_found_is_404() {
    // given
    let fx = setup(Answer::Action(ActionError::NotFound("gone".to_string())));
    // when
    let got = label(&fx, json!({"hash": HASH, "for": "x"})).await;
    // then
    assert_eq!(line(got), "404 not_found");
}

#[tokio::test]
async fn other_methods_are_405() {
    // given
    let fx = setup(Answer::List(sample()));
    // when / then
    let post = send(&fx, "POST", "/api/network/invites", "{}").await;
    let get = send(&fx, "GET", "/api/network/invites/label", "").await;
    let put = send(&fx, "PUT", "/api/network/invites/label", "{}").await;
    assert_eq!(post.0, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(get.0, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(put.0, StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn slot_route_reaches_the_slot() {
    // given
    let fx = setup(Answer::List(sample()));
    // when
    let list = send(&fx, "GET", "/api/net/main/network/invites", "").await;
    let edit = send(
        &fx,
        "POST",
        "/api/net/main/network/invites/label",
        &json!({"hash": HASH, "for": "x"}).to_string(),
    )
    .await;
    // then
    assert_eq!(list.0, StatusCode::OK);
    assert_eq!(edit.0, StatusCode::OK);
    assert_eq!(fx.seen.lock().unwrap().len(), 1);
}
