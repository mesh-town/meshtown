//! The `forget` and `unforget` member routes, under `/api/members` (the main network) and under
//! `/api/net/{slot}/members` (the network of a slot): the id reaches the right node as the right action,
//! the node's refusals come back as 409 with their code, bad bodies and unknown slots never reach a node.

use axum::http::{Method, StatusCode};
use serde_json::{json, Value};

mod main_network {
    use super::*;

    #[tokio::test]
    async fn forget_passes_the_id_to_the_node_as_forget() {
        // given
        let w = setup(None);
        // when
        let (status, body) = w.post("/api/members/forget", &id_body()).await;
        // then
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(w.main.calls(), vec![MemberActionReq::Forget(id())]);
    }

    #[tokio::test]
    async fn unforget_passes_the_id_to_the_node_as_unforget() {
        // given
        let w = setup(None);
        // when
        let (status, body) = w.post("/api/members/unforget", &id_body()).await;
        // then
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(w.main.calls(), vec![MemberActionReq::Unforget(id())]);
    }

    #[tokio::test]
    async fn the_id_is_trimmed_before_it_reaches_the_node() {
        // given
        let w = setup(None);
        // when
        let body = json!({"id": format!("  {}\n", id())}).to_string();
        let (status, _) = w.post("/api/members/forget", &body).await;
        // then
        assert_eq!(status, StatusCode::OK);
        assert_eq!(w.main.calls(), vec![MemberActionReq::Forget(id())]);
    }

    #[tokio::test]
    async fn reply_is_the_member_list_of_the_node() {
        // given
        let w = setup(None);
        // when
        let (_, body) = w.post("/api/members/forget", &id_body()).await;
        // then
        assert_eq!(body["me"], "me");
        assert_eq!(body["counts"]["forgotten"], 0);
        assert_eq!(body["me_removed"], false);
    }

    #[tokio::test]
    async fn only_the_main_node_is_asked() {
        // given
        let w = setup(None);
        // when
        w.post("/api/members/forget", &id_body()).await;
        // then
        assert!(w.slot.calls().is_empty(), "slot node: {:?}", w.slot.calls());
    }
}

mod node_refuses {
    use super::*;

    #[tokio::test]
    async fn forget_of_an_online_member_is_409_online() {
        // given
        let w = setup(Some("online"));
        // when
        let (status, body) = w.post("/api/members/forget", &id_body()).await;
        // then
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"], "online");
        assert_eq!(body["message"], MESSAGE);
    }

    #[tokio::test]
    async fn unforget_of_a_member_not_forgotten_is_409_not_forgotten() {
        // given
        let w = setup(Some("not_forgotten"));
        // when
        let (status, body) = w.post("/api/members/unforget", &id_body()).await;
        // then
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"], "not_forgotten");
        assert_eq!(body["message"], MESSAGE);
    }
}

mod bad_request {
    use super::*;

    #[tokio::test]
    async fn body_without_id_is_400() {
        for action in ACTIONS {
            // given
            let w = setup(None);
            // when
            let (status, body) = w
                .post(&format!("/api/members/{action}"), r#"{"confirm":true}"#)
                .await;
            // then
            assert_eq!(status, StatusCode::BAD_REQUEST, "{action}");
            assert_eq!(body["error"], "bad_request", "{action}");
            assert!(w.main.calls().is_empty(), "{action}: node was called");
        }
    }

    #[tokio::test]
    async fn blank_or_non_string_id_is_400() {
        for bad in [json!(""), json!("   "), json!(5), json!(null), json!(["a"])] {
            for action in ACTIONS {
                // given
                let w = setup(None);
                // when
                let body = json!({ "id": bad }).to_string();
                let (status, reply) = w.post(&format!("/api/members/{action}"), &body).await;
                // then
                assert_eq!(status, StatusCode::BAD_REQUEST, "{action} with id {bad}");
                assert_eq!(reply["error"], "bad_request", "{action} with id {bad}");
                assert!(w.main.calls().is_empty(), "{action} with id {bad}");
            }
        }
    }

    #[tokio::test]
    async fn body_that_is_not_a_json_object_is_400() {
        for text in ["[]", "\"x\"", "null", "42", "{not json", "id=abc", ""] {
            for action in ACTIONS {
                // given
                let w = setup(None);
                // when
                let (status, reply) = w.post(&format!("/api/members/{action}"), text).await;
                // then
                assert_eq!(
                    status,
                    StatusCode::BAD_REQUEST,
                    "{action} with body {text:?}"
                );
                assert_eq!(reply["error"], "bad_request", "{action} with body {text:?}");
                assert!(w.main.calls().is_empty(), "{action} with body {text:?}");
            }
        }
    }

    #[tokio::test]
    async fn get_is_405_bad_method() {
        for action in ACTIONS {
            // given
            let w = setup(None);
            // when
            let (status, body) = w
                .request(Method::GET, &format!("/api/members/{action}"))
                .await;
            // then
            assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{action}");
            assert_eq!(body["error"], "bad_method", "{action}");
            assert!(w.main.calls().is_empty(), "{action}: node was called");
        }
    }
}

mod slot_prefix {
    use super::*;

    #[tokio::test]
    async fn forget_reaches_the_node_of_the_slot() {
        // given
        let w = setup(None);
        // when
        let (status, body) = w.post(&format!("{SLOT}/members/forget"), &id_body()).await;
        // then
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(w.slot.calls(), vec![MemberActionReq::Forget(id())]);
    }

    #[tokio::test]
    async fn unforget_reaches_the_node_of_the_slot() {
        // given
        let w = setup(None);
        // when
        let (status, body) = w
            .post(&format!("{SLOT}/members/unforget"), &id_body())
            .await;
        // then
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(w.slot.calls(), vec![MemberActionReq::Unforget(id())]);
    }

    #[tokio::test]
    async fn the_main_node_is_not_asked() {
        // given
        let w = setup(None);
        // when
        w.post(&format!("{SLOT}/members/forget"), &id_body()).await;
        // then
        assert!(w.main.calls().is_empty(), "main node: {:?}", w.main.calls());
    }

    #[tokio::test]
    async fn refusals_of_the_slot_node_are_409_with_their_code() {
        for (action, code) in [("forget", "online"), ("unforget", "not_forgotten")] {
            // given
            let w = setup(Some(code));
            // when
            let (status, body) = w
                .post(&format!("{SLOT}/members/{action}"), &id_body())
                .await;
            // then
            assert_eq!(status, StatusCode::CONFLICT, "{action}");
            assert_eq!(body["error"], code, "{action}");
        }
    }

    #[tokio::test]
    async fn unknown_slot_is_404_unknown_net_and_no_node_is_asked() {
        for action in ACTIONS {
            // given
            let w = setup(None);
            // when
            let path = format!("/api/net/zz2c3d/members/{action}");
            let (status, body) = w.post(&path, &id_body()).await;
            // then
            assert_eq!(status, StatusCode::NOT_FOUND, "{action}");
            assert_eq!(body["error"], "unknown_net", "{action}");
            assert!(w.main.calls().is_empty(), "{action}: main node was called");
            assert!(w.slot.calls().is_empty(), "{action}: slot node was called");
        }
    }

    #[tokio::test]
    async fn bad_body_never_reaches_the_node_of_the_slot() {
        // given
        let w = setup(None);
        // when
        let (status, body) = w.post(&format!("{SLOT}/members/forget"), "{}").await;
        // then
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "bad_request");
        assert!(w.slot.calls().is_empty(), "slot node was called");
    }
}

// --- setup and helpers ---

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use crate::config::config_file::Settings;
use crate::web_gui::members::dto::{Counts, MembersReply};
use crate::web_gui::test_host::{config_with_key, send, slot_handle, test_app, FakeHost};
use crate::web_gui::{
    ActionError, LogPage, MemberActionReq, MemberError, MemberFuture, NodeControl, NodeSnapshot,
};

const ACTIONS: [&str; 2] = ["forget", "unforget"];
const SLOT: &str = "/api/net/ab2c3d";
const MESSAGE: &str = "refused by the node";

fn id() -> String {
    "ab".repeat(32)
}

fn id_body() -> String {
    json!({ "id": id() }).to_string()
}

fn empty_list() -> MembersReply {
    MembersReply {
        me: "me".to_string(),
        me_excluded: false,
        me_removed: false,
        me_pending: false,
        need: 1,
        me_admission: None,
        counts: Counts {
            members: 1,
            voting: 0,
            excluded: 0,
            unlisted: 0,
            pending: 0,
            forgotten: 0,
        },
        members: Vec::new(),
        changed: Some(true),
    }
}

/// A node that records the member actions it is asked for and answers each with the list, or with the
/// refusal `code` when one is set.
struct Recorder {
    refusal: Option<&'static str>,
    calls: Mutex<Vec<MemberActionReq>>,
}

impl Recorder {
    fn new(refusal: Option<&'static str>) -> Arc<Recorder> {
        Arc::new(Recorder {
            refusal,
            calls: Mutex::new(Vec::new()),
        })
    }

    fn calls(&self) -> Vec<MemberActionReq> {
        self.calls.lock().unwrap().clone()
    }
}

impl NodeControl for Recorder {
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
    fn apply_config(&self, _settings: &Settings, _paths: &[&'static str]) {}
    fn switch_network(&self, _key: &str) {}
    fn stop(&self) {}

    fn member_action(&self, act: MemberActionReq) -> MemberFuture {
        self.calls.lock().unwrap().push(act);
        let answer: Pin<Box<dyn Future<Output = _> + Send>> = match self.refusal {
            Some(code) => Box::pin(std::future::ready(Err(MemberError::Refused {
                code,
                message: MESSAGE.to_string(),
            }))),
            None => Box::pin(std::future::ready(Ok(empty_list()))),
        };
        answer
    }
}

/// A host with the main network and the slot `ab2c3d`, each over its own recording node.
struct World {
    router: axum::Router,
    main: Arc<Recorder>,
    slot: Arc<Recorder>,
}

impl World {
    async fn post(&self, path: &str, body: &str) -> (StatusCode, Value) {
        send(&self.router, Method::POST, path, body).await
    }

    async fn request(&self, method: Method, path: &str) -> (StatusCode, Value) {
        send(&self.router, method, path, "").await
    }
}

/// Both nodes answer with `refusal` when it is given, otherwise with the member list.
fn setup(refusal: Option<&'static str>) -> World {
    let main = Recorder::new(refusal);
    let slot = Recorder::new(refusal);
    let handles = vec![
        slot_handle("main", main.clone(), config_with_key(Some("k"))),
        slot_handle("ab2c3d", slot.clone(), config_with_key(Some("k"))),
    ];
    let host = FakeHost::new(handles);
    World {
        router: test_app(host).0,
        main,
        slot,
    }
}
