//! Shared fixtures of the member-list API tests: the fake node, the router fixture over it, request and
//! reply helpers, and the documented example list with its JSON.

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::web_gui::auth::state::AuthState;
use crate::web_gui::config_api::ConfigState;
use crate::web_gui::control::{
    ActionError, MemberActionReq, MemberError, MemberFuture, NodeControl,
};
use crate::web_gui::http::ConnInfo;
use crate::web_gui::members::dto::{
    Counts, ExcludedInfo, MemberRow, MembersReply, RefusalInfo, VoteInfo,
};
use crate::web_gui::single_slot_host;
use crate::web_gui::{App, LogPage, NodeSnapshot, WebShared};

const PORT: u16 = 18766;
pub(super) const TOKEN: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ0123456789ABCDEFGHJK";
pub(super) const ACTIONS: [&str; 5] = ["exclude", "unexclude", "block", "unblock", "leave"];

/// What the fake node answers to `members` and `member_action`.
pub(super) enum Answer {
    List(MembersReply),
    Refused {
        code: &'static str,
        message: &'static str,
    },
    Action(ActionError),
}

/// Actions the node was asked for, as `exclude:<id>`, …, `leave`.
type Seen = Arc<Mutex<Vec<String>>>;

/// Answers by `answer` and remembers every action it was asked for.
struct FakeNode {
    answer: Answer,
    seen: Seen,
}

impl FakeNode {
    fn reply(&self) -> Result<MembersReply, MemberError> {
        match &self.answer {
            Answer::List(reply) => Ok(reply.clone()),
            Answer::Refused { code, message } => Err(MemberError::Refused {
                code,
                message: message.to_string(),
            }),
            Answer::Action(err) => Err(MemberError::Action(err.clone())),
        }
    }
}

/// A node with no member overrides: the trait defaults answer.
struct BareNode;

macro_rules! required_methods {
    () => {
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
    };
}

impl NodeControl for FakeNode {
    required_methods!();

    fn members(&self) -> MemberFuture {
        let reply = self.reply();
        Box::pin(async move { reply })
    }

    fn member_action(&self, act: MemberActionReq) -> MemberFuture {
        let seen = match act {
            MemberActionReq::Exclude { id, reason: None } => format!("exclude:{id}"),
            MemberActionReq::Exclude {
                id,
                reason: Some(r),
            } => format!("exclude:{id}:{r}"),
            MemberActionReq::Unexclude(id) => format!("unexclude:{id}"),
            MemberActionReq::Admit(id) => format!("admit:{id}"),
            MemberActionReq::Unadmit(id) => format!("unadmit:{id}"),
            MemberActionReq::Block(id) => format!("block:{id}"),
            MemberActionReq::Unblock(id) => format!("unblock:{id}"),
            MemberActionReq::Leave => "leave".to_string(),
        };
        self.seen.lock().unwrap().push(seen);
        let reply = self.reply();
        Box::pin(async move { reply })
    }
}

impl NodeControl for BareNode {
    required_methods!();
}

pub(super) struct Fixture {
    pub(super) router: Router,
    pub(super) app: Arc<App>,
    seen: Seen,
}

impl Fixture {
    pub(super) async fn get(&self, path: &str) -> (StatusCode, Value) {
        send(&self.router, "GET", path, "", Some(TOKEN)).await
    }

    pub(super) async fn post(&self, path: &str, body: &str) -> (StatusCode, Value) {
        send(&self.router, "POST", path, body, Some(TOKEN)).await
    }

    /// The request for `action` with the body it needs: a node id, or the confirmation for `leave`.
    pub(super) async fn post_action(&self, action: &str) -> (StatusCode, Value) {
        let body = if action == "leave" {
            r#"{"confirm":true}"#.to_string()
        } else {
            id_body()
        };
        self.post(&format!("/api/members/{action}"), &body).await
    }

    pub(super) fn seen(&self) -> Vec<String> {
        self.seen.lock().unwrap().clone()
    }
}

pub(super) fn setup(answer: Answer) -> Fixture {
    let seen = Seen::default();
    fixture(
        Arc::new(FakeNode {
            answer,
            seen: seen.clone(),
        }),
        seen,
    )
}

/// A node that leaves the member methods to the trait defaults.
pub(super) fn setup_bare() -> Fixture {
    fixture(Arc::new(BareNode), Seen::default())
}

/// The real router and `App` over `node`; loopback access mode, no files, no sockets.
fn fixture(node: Arc<dyn NodeControl>, seen: Seen) -> Fixture {
    use crate::config::config_file::{prepare, CliOverrides, ConfigSource, ListenAccess};
    let auth = AuthState::new("mesh_test".to_string(), None, ListenAccess::Localhost);
    let run = prepare(&ConfigSource::Disabled, &CliOverrides::default(), false, 0)
        .expect("prepare without a file");
    let app = Arc::new(App {
        shared: Arc::new(WebShared::new(PORT, TOKEN.to_string(), auth)),
        host: single_slot_host(node, ConfigState::from_run(&run)),
        token_dir: None,
        stopping: std::sync::atomic::AtomicBool::new(false),
    });
    Fixture {
        router: crate::web_gui::routes::router(app.clone()),
        app,
        seen,
    }
}

/// One request from the loopback client; `token: None` sends no credentials.
pub(super) async fn send(
    router: &Router,
    method: &str,
    path: &str,
    body: &str,
    token: Option<&str>,
) -> (StatusCode, Value) {
    let host = format!("127.0.0.1:{PORT}");
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header("host", host.as_str());
    req = req
        .header("origin", format!("http://{host}"))
        .extension(ConnInfo::loopback(PORT));
    if let Some(token) = token {
        req = req.header("x-meshtown-token", token);
    }
    let resp = router
        .clone()
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// A reply as `"<status> <error code>"` (`-` when the body has no `error`).
pub(super) fn line((status, body): (StatusCode, Value)) -> String {
    format!(
        "{} {}",
        status.as_u16(),
        body["error"].as_str().unwrap_or("-")
    )
}

/// A 64-hex node id padded from a short prefix.
fn full(prefix: &str) -> String {
    format!("{prefix}{}", "0".repeat(64 - prefix.len()))
}

pub(super) fn target() -> String {
    "ab".repeat(32)
}

pub(super) fn id_body() -> String {
    json!({"id": target()}).to_string()
}

fn row(prefix: &str, name: Option<&str>, status: &'static str) -> MemberRow {
    MemberRow {
        id: prefix.to_string(),
        id_full: full(prefix),
        name: name.map(str::to_string),
        is_self: false,
        online: true,
        status,
        vote: None,
        excluded: None,
        blocked_by_me: false,
        denied_by_me: None,
        last_refusal: None,
        admission: None,
    }
}

fn sample_counts() -> Counts {
    Counts {
        members: 3,
        voting: 1,
        excluded: 1,
        unlisted: 1,
        pending: 0,
    }
}

/// The documented example: me, a member with my pending vote, an excluded node.
pub(super) fn sample_reply() -> MembersReply {
    let (me, router) = (full("a1b2c3d4"), full("e5f6a7b8"));
    let voting = VoteInfo {
        votes: 1,
        need: 2,
        proposed_by: me.clone(),
        voters: vec![me.clone()],
        mine: true,
        reason: Some("noisy link".to_string()),
    };
    let gone = ExcludedInfo {
        how: "votes",
        at_unix: 1_792_400_000,
        voters: vec![me.clone(), router],
    };
    MembersReply {
        me,
        me_excluded: false,
        me_pending: false,
        me_admission: None,
        need: 2,
        // plain input here: the DTO does not derive the counts from the rows
        counts: sample_counts(),
        members: vec![
            MemberRow {
                is_self: true,
                ..row("a1b2c3d4", Some("laptop"), "member")
            },
            MemberRow {
                vote: Some(voting),
                denied_by_me: Some("voted_by_me"),
                ..row("e5f6a7b8", Some("router"), "member")
            },
            MemberRow {
                online: false,
                excluded: Some(gone),
                denied_by_me: Some("excluded"),
                last_refusal: Some(RefusalInfo {
                    side: "me",
                    reason: "excluded",
                    at_unix: 1_792_400_100,
                }),
                ..row("c9d0e1f2", None, "excluded")
            },
        ],
        changed: None,
    }
}

/// `sample_reply` as the API documents it, written out key by key.
pub(super) fn golden() -> Value {
    let (me, router) = (full("a1b2c3d4"), full("e5f6a7b8"));
    json!({
        "me": me, "me_excluded": false, "me_pending": false, "need": 2, "me_admission": null,
        "counts": {"members": 3, "voting": 1, "excluded": 1, "unlisted": 1, "pending": 0},
        "members": [
            {"id": "a1b2c3d4", "id_full": me, "name": "laptop", "self": true, "online": true,
             "status": "member", "vote": null, "excluded": null,
             "blocked_by_me": false, "denied_by_me": null, "last_refusal": null, "admission": null},
            {"id": "e5f6a7b8", "id_full": router, "name": "router", "self": false, "online": true,
             "status": "member",
             "vote": {"votes": 1, "need": 2, "proposed_by": me, "voters": [me], "mine": true, "reason": "noisy link"},
             "excluded": null, "blocked_by_me": false, "denied_by_me": "voted_by_me", "last_refusal": null, "admission": null},
            {"id": "c9d0e1f2", "id_full": full("c9d0e1f2"), "name": null, "self": false, "online": false,
             "status": "excluded", "vote": null,
             "excluded": {"how": "votes", "at_unix": 1_792_400_000u64, "voters": [me, router]},
             "blocked_by_me": false, "denied_by_me": "excluded",
             "last_refusal": {"side": "me", "reason": "excluded", "at_unix": 1_792_400_100u64}, "admission": null}
        ]
    })
}
