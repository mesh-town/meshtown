//! Notices of the page: the `notices` list at the top of `/api/state` and `/api/overview`, and its
//! acknowledgement by `POST /api/notices/ack`.

// assumed: this file is declared next to the handlers (`state_host.rs`) as
// `#[cfg(test)] #[path = "notices_tests.rs"] mod notices_tests;`.
// assumed: `HostControl::notices() -> Vec<Notice>` and `HostControl::ack_notice(u64) -> bool` are the trait
// methods with the defaults of the plan. The list behind the fake host is the fixture's own: the real
// one is tested in `node/host/notices_tests.rs`.

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::config::config_file::NetSlotId;
use crate::transport::net::contract::pid;
use crate::web_gui::http::ConnInfo;
use crate::web_gui::test_host::{
    config_with_key, send, slot_handle, test_app, FakeHost, SnapNode, PORT,
};
use crate::web_gui::{AddedSlot, HostControl, HostError, Notice, SlotHandle};

const OTHER_SLOT: &str = "ab2c3d";

mod without_notices {
    use super::*;

    #[tokio::test]
    async fn state_has_an_empty_list() {
        let f = setup();
        let (status, state) = f.get("/api/state").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(state["notices"], json!([]));
    }

    #[tokio::test]
    async fn overview_has_an_empty_list() {
        let f = setup();
        let (status, overview) = f.get("/api/overview").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(overview["notices"], json!([]));
    }

    #[tokio::test]
    async fn the_state_of_a_slot_has_an_empty_list() {
        let f = setup();
        let (_, state) = f.get(&format!("/api/net/{OTHER_SLOT}/state")).await;
        assert_eq!(state["notices"], json!([]));
    }
}

mod after_a_network_was_deleted {
    use super::*;

    #[tokio::test]
    async fn state_lists_the_notice_with_every_field() {
        // given
        let f = setup();
        let id = f.push(
            Some("alpha"),
            Some("ab12cd34"),
            2,
            Some("Dana"),
            1_700_000_000,
        );
        // when
        let (_, state) = f.get("/api/state").await;
        // then
        let notices = state["notices"].as_array().expect("a list");
        assert_eq!(notices.len(), 1, "{state}");
        let note = notices[0].as_object().expect("an object");
        let mut keys: Vec<&str> = note.keys().map(String::as_str).collect();
        keys.sort_unstable();
        let want = ["at_unix", "by", "by_name", "id", "id_hex", "kind", "label"];
        assert_eq!(keys, want, "exactly the fields of the notice");
        assert_eq!(note["id"], json!(id));
        assert!(note["id"].is_u64(), "the id is a number: {note:?}");
        assert_eq!(note["kind"], "net_deleted");
        assert_eq!(note["label"], "alpha");
        assert_eq!(note["id_hex"], "ab12cd34");
        assert_eq!(note["by"], json!(pid(2).to_string()));
        assert_eq!(note["by_name"], "Dana");
        assert_eq!(note["at_unix"], 1_700_000_000);
    }

    #[tokio::test]
    async fn names_that_are_not_known_are_null() {
        let f = setup();
        f.push(None, None, 2, None, 5);
        let (_, state) = f.get("/api/state").await;
        let note = &state["notices"][0];
        assert_eq!(note["label"], Value::Null);
        assert_eq!(note["id_hex"], Value::Null);
        assert_eq!(note["by_name"], Value::Null);
    }

    #[tokio::test]
    async fn the_notices_go_from_the_oldest_to_the_newest() {
        let f = setup();
        let first = f.push(Some("one"), Some("aa"), 2, None, 1);
        let second = f.push(Some("two"), Some("bb"), 2, None, 2);
        let (_, state) = f.get("/api/state").await;
        assert_eq!(state["notices"][0]["id"], json!(first));
        assert_eq!(state["notices"][1]["id"], json!(second));
    }

    #[tokio::test]
    async fn overview_and_every_state_carry_the_same_list() {
        // given
        let f = setup();
        f.push(Some("alpha"), Some("ab12cd34"), 2, None, 9);
        // when
        let (_, state) = f.get("/api/state").await;
        let (_, overview) = f.get("/api/overview").await;
        let (_, slot_state) = f.get(&format!("/api/net/{OTHER_SLOT}/state")).await;
        // then
        assert_eq!(state["notices"].as_array().map(Vec::len), Some(1));
        assert_eq!(overview["notices"], state["notices"]);
        assert_eq!(slot_state["notices"], state["notices"]);
    }
}

mod acknowledging {
    use super::*;

    #[tokio::test]
    async fn answers_an_empty_object_and_removes_the_notice() {
        // given
        let f = setup();
        let id = f.push(Some("alpha"), Some("ab12cd34"), 2, None, 5);
        // when
        let (status, body) = f.ack(json!({ "id": id })).await;
        // then
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body, json!({}));
        let (_, state) = f.get("/api/state").await;
        assert_eq!(state["notices"], json!([]));
        let (_, overview) = f.get("/api/overview").await;
        assert_eq!(overview["notices"], json!([]));
    }

    #[tokio::test]
    async fn keeps_the_other_notices() {
        let f = setup();
        let first = f.push(Some("one"), Some("aa"), 2, None, 1);
        let second = f.push(Some("two"), Some("bb"), 2, None, 2);
        let (status, _) = f.ack(json!({ "id": first })).await;
        assert_eq!(status, StatusCode::OK);
        let (_, state) = f.get("/api/state").await;
        let left: Vec<&Value> = state["notices"].as_array().unwrap().iter().collect();
        assert_eq!(left.len(), 1, "{state}");
        assert_eq!(left[0]["id"], json!(second));
    }

    #[tokio::test]
    async fn an_unknown_id_is_not_found_and_changes_nothing() {
        // given
        let f = setup();
        let id = f.push(Some("alpha"), Some("ab12cd34"), 2, None, 5);
        // when
        let (status, body) = f.ack(json!({ "id": id + 100 })).await;
        // then
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
        assert_eq!(body["error"], "not_found");
        assert_eq!(f.book_len(), 1);
    }

    #[tokio::test]
    async fn the_second_ack_of_the_same_id_is_not_found() {
        let f = setup();
        let id = f.push(Some("alpha"), Some("ab12cd34"), 2, None, 5);
        f.ack(json!({ "id": id })).await;
        let (status, body) = f.ack(json!({ "id": id })).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
        assert_eq!(body["error"], "not_found");
    }

    #[tokio::test]
    async fn a_body_without_a_numeric_id_is_invalid() {
        for body in [
            json!({}),
            json!({ "id": "x" }),
            json!({ "id": null }),
            json!({ "id": -1 }),
            json!({ "id": 1.5 }),
        ] {
            // given
            let f = setup();
            f.push(Some("alpha"), Some("ab12cd34"), 2, None, 5);
            // when
            let (status, reply) = f.ack(body.clone()).await;
            // then
            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}: {reply}");
            assert_eq!(reply["error"], "invalid", "{body}");
            let fields = reply["fields"].as_array().expect("fields is a list");
            assert_eq!(fields.len(), 1, "{body}: {reply}");
            assert_eq!(fields[0]["path"], "id", "{body}");
            assert!(
                fields[0]["message"].as_str().is_some_and(|m| !m.is_empty()),
                "{body}: the field has a message: {reply}"
            );
            assert_eq!(f.book_len(), 1, "{body}: nothing is removed");
        }
    }

    #[tokio::test]
    async fn get_is_not_allowed() {
        let f = setup();
        let (status, body) = f.get("/api/notices/ack").await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(body["error"], "bad_method");
    }

    #[tokio::test]
    async fn without_a_token_is_refused_and_removes_nothing() {
        // given
        let f = setup();
        let id = f.push(Some("alpha"), Some("ab12cd34"), 2, None, 5);
        // when
        let (status, body) = f.post_without_token(json!({ "id": id })).await;
        // then
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
        assert_eq!(body["error"], "bad_token");
        assert_eq!(f.book_len(), 1, "the notice stays");
    }
}

// --- setup and helpers ---

/// A router over a host with two slots; the host keeps a list of notices.
struct Fixture {
    router: axum::Router,
    host: Arc<NoticeHost>,
}

fn setup() -> Fixture {
    let slots = ["main", OTHER_SLOT]
        .iter()
        .map(|id| slot_handle(id, SnapNode::new(Default::default()), config_with_key(None)))
        .collect();
    let host = Arc::new(NoticeHost {
        inner: FakeHost::new(slots),
        book: Mutex::new(Vec::new()),
    });
    let router = test_app(host.clone()).0;
    Fixture { router, host }
}

impl Fixture {
    /// Notes a deleted network on the host; returns the id of the notice.
    fn push(
        &self,
        label: Option<&str>,
        id_hex: Option<&str>,
        admin_seed: u8,
        admin_name: Option<&str>,
        at_unix: u64,
    ) -> u64 {
        let mut book = self.host.book.lock().unwrap();
        let id = book.last().map_or(1, |n| n.id + 1);
        book.push(Notice {
            id,
            kind: "net_deleted",
            label: label.map(str::to_string),
            id_hex: id_hex.map(str::to_string),
            by: pid(admin_seed).to_string(),
            by_name: admin_name.map(str::to_string),
            at_unix,
        });
        id
    }

    fn book_len(&self) -> usize {
        self.host.book.lock().unwrap().len()
    }

    async fn get(&self, path: &str) -> (StatusCode, Value) {
        send(&self.router, Method::GET, path, "").await
    }

    async fn ack(&self, body: Value) -> (StatusCode, Value) {
        send(
            &self.router,
            Method::POST,
            "/api/notices/ack",
            &body.to_string(),
        )
        .await
    }

    /// The same request as `ack` from the loopback client, without the page token.
    async fn post_without_token(&self, body: Value) -> (StatusCode, Value) {
        let host = format!("127.0.0.1:{PORT}");
        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/notices/ack")
            .header("host", host.as_str())
            .header("origin", format!("http://{host}"))
            .extension(ConnInfo::loopback(PORT))
            .body(Body::from(body.to_string()))
            .unwrap();
        let resp = self.router.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }
}

/// The fake host of the other route tests plus a list of notices.
struct NoticeHost {
    inner: Arc<FakeHost>,
    book: Mutex<Vec<Notice>>,
}

impl HostControl for NoticeHost {
    fn slot(&self, id: &NetSlotId) -> Option<SlotHandle> {
        self.inner.slot(id)
    }

    fn main_slot(&self) -> SlotHandle {
        self.inner.main_slot()
    }

    fn slots(&self) -> Vec<SlotHandle> {
        self.inner.slots()
    }

    fn stop_all(&self) {
        self.inner.stop_all();
    }

    fn add_slot(&self) -> Result<AddedSlot, HostError> {
        self.inner.add_slot()
    }

    fn retire_slot(&self, id: &NetSlotId, announced: bool) -> Result<(), HostError> {
        self.inner.retire_slot(id, announced)
    }

    fn notices(&self) -> Vec<Notice> {
        self.book.lock().unwrap().clone()
    }

    fn ack_notice(&self, id: u64) -> bool {
        let mut book = self.book.lock().unwrap();
        let before = book.len();
        book.retain(|n| n.id != id);
        book.len() != before
    }
}
