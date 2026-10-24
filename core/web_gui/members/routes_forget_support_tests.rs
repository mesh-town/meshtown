//! Shared setup of the forget route tests: a host with the main network and one slot, each over its own
//! recording node.

use axum::http::{Method, StatusCode};
use serde_json::{json, Value};

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use crate::config::config_file::Settings;
use crate::web_gui::members::dto::{Counts, MembersReply};
use crate::web_gui::test_host::{config_with_key, send, slot_handle, test_app, FakeHost};
use crate::web_gui::{
    ActionError, LogPage, MemberActionReq, MemberError, MemberFuture, NodeControl, NodeSnapshot,
};

pub(super) const ACTIONS: [&str; 2] = ["forget", "unforget"];
pub(super) const SLOT: &str = "/api/net/ab2c3d";
pub(super) const MESSAGE: &str = "refused by the node";

pub(super) fn id() -> String {
    "ab".repeat(32)
}

pub(super) fn id_body() -> String {
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
pub(super) struct Recorder {
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

    pub(super) fn calls(&self) -> Vec<MemberActionReq> {
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
pub(super) struct World {
    router: axum::Router,
    pub(super) main: Arc<Recorder>,
    pub(super) slot: Arc<Recorder>,
}

impl World {
    pub(super) async fn post(&self, path: &str, body: &str) -> (StatusCode, Value) {
        send(&self.router, Method::POST, path, body).await
    }

    pub(super) async fn request(&self, method: Method, path: &str) -> (StatusCode, Value) {
        send(&self.router, method, path, "").await
    }
}

/// Both nodes answer with `refusal` when it is given, otherwise with the member list.
pub(super) fn setup(refusal: Option<&'static str>) -> World {
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
