//! Test doubles for the multi-slot page: a host that records the calls it gets, a node with a set
//! snapshot, and a router fixture over them.

// The route tests of other files pick what they need from this shared kit.
#![allow(dead_code)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

use super::auth::state::AuthState;
use super::control::{
    ActionError, InviteJoinView, MemberActionReq, MemberError, MemberFuture, NodeControl,
};
use super::http::ConnInfo;
use super::{
    AddedSlot, App, ConfigState, HostControl, HostError, LogPage, MembersReply, NodeSnapshot,
    SlotHandle, WebShared,
};
use crate::config::config_file::NetSlotId;
use crate::proxy::ProxyFront;

pub(crate) const PORT: u16 = 18767;
pub(crate) const TOKEN: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ0123456789ABCDEFGHJK";

/// A call the page made on the host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum HostCall {
    Add,
    Retire(NetSlotId, bool),
    StopAll,
    ClaimProxy(NetSlotId),
}

struct State {
    calls: Vec<HostCall>,
    add: Result<AddedSlot, HostError>,
    retire: Result<(), HostError>,
    broken: Vec<NetSlotId>,
    claim: Result<(), HostError>,
    front: Option<ProxyFront>,
    proxy_net: NetSlotId,
    check_lock_free_of: Option<NetSlotId>,
    lock_was_free: Option<bool>,
}

/// A host over fixed slots with settable answers.
pub(crate) struct FakeHost {
    slots: Vec<SlotHandle>,
    state: Mutex<State>,
}

impl FakeHost {
    pub(crate) fn new(slots: Vec<SlotHandle>) -> Arc<FakeHost> {
        Arc::new(FakeHost {
            slots,
            state: Mutex::new(State {
                calls: Vec::new(),
                add: Err(HostError::TooManyNets),
                retire: Ok(()),
                broken: Vec::new(),
                claim: Ok(()),
                front: None,
                proxy_net: NetSlotId::main(),
                check_lock_free_of: None,
                lock_was_free: None,
            }),
        })
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn calls(&self) -> Vec<HostCall> {
        self.state().calls.clone()
    }

    pub(crate) fn set_add(&self, answer: Result<AddedSlot, HostError>) {
        self.state().add = answer;
    }

    pub(crate) fn set_retire(&self, answer: Result<(), HostError>) {
        self.state().retire = answer;
    }

    pub(crate) fn set_broken(&self, ids: Vec<NetSlotId>) {
        self.state().broken = ids;
    }

    pub(crate) fn set_claim_result(&self, answer: Result<(), HostError>) {
        self.state().claim = answer;
    }

    pub(crate) fn set_front(&self, front: Option<ProxyFront>) {
        self.state().front = front;
    }

    pub(crate) fn set_proxy_net(&self, id: NetSlotId) {
        self.state().proxy_net = id;
    }

    /// On `claim_proxy`, try this slot's config lock and remember whether it was free.
    pub(crate) fn check_lock_free_of(&self, id: NetSlotId) {
        self.state().check_lock_free_of = Some(id);
    }

    /// `Some(true)` if the watched slot's config lock was free during the last `claim_proxy`.
    pub(crate) fn lock_was_free(&self) -> Option<bool> {
        self.state().lock_was_free
    }
}

impl HostControl for FakeHost {
    fn slot(&self, id: &NetSlotId) -> Option<SlotHandle> {
        self.slots.iter().find(|s| s.id == *id).cloned()
    }

    fn main_slot(&self) -> SlotHandle {
        self.slot(&NetSlotId::main()).expect("a main slot")
    }

    fn slots(&self) -> Vec<SlotHandle> {
        self.slots.clone()
    }

    fn stop_all(&self) {
        self.state().calls.push(HostCall::StopAll);
    }

    fn add_slot(&self) -> Result<AddedSlot, HostError> {
        let mut st = self.state();
        st.calls.push(HostCall::Add);
        st.add.clone()
    }

    fn retire_slot(&self, id: &NetSlotId, announced: bool) -> Result<(), HostError> {
        let mut st = self.state();
        st.calls.push(HostCall::Retire(id.clone(), announced));
        st.retire.clone()
    }

    fn broken(&self) -> Vec<NetSlotId> {
        self.state().broken.clone()
    }

    fn claim_proxy(&self, id: &NetSlotId) -> Result<(), HostError> {
        let watched = self.state().check_lock_free_of.clone();
        let free = watched
            .and_then(|w| self.slot(&w))
            .map(|s| s.config.try_lock().is_ok());
        let mut st = self.state();
        st.calls.push(HostCall::ClaimProxy(id.clone()));
        st.lock_was_free = free.or(st.lock_was_free);
        st.claim.clone()
    }

    fn proxy_net(&self) -> NetSlotId {
        self.state().proxy_net.clone()
    }

    fn proxy_front(&self) -> Option<ProxyFront> {
        self.state().front.clone()
    }
}

/// A node answering with a set snapshot; counts the calls the tests care about.
pub(crate) struct SnapNode {
    pub(crate) snapshot: Mutex<NodeSnapshot>,
    pub(crate) invite_join: Mutex<Option<InviteJoinView>>,
    pub(crate) members: Mutex<Result<MembersReply, MemberError>>,
    pub(crate) member_calls: AtomicUsize,
    pub(crate) join_result: Mutex<Result<(), ActionError>>,
    pub(crate) joins: AtomicUsize,
    pub(crate) restarts: AtomicUsize,
    pub(crate) stops: AtomicUsize,
}

impl SnapNode {
    pub(crate) fn new(snapshot: NodeSnapshot) -> Arc<SnapNode> {
        Arc::new(SnapNode {
            snapshot: Mutex::new(snapshot),
            invite_join: Mutex::new(None),
            members: Mutex::new(Err(MemberError::Action(ActionError::NotFound(
                "no members set".to_string(),
            )))),
            member_calls: AtomicUsize::new(0),
            join_result: Mutex::new(Ok(())),
            joins: AtomicUsize::new(0),
            restarts: AtomicUsize::new(0),
            stops: AtomicUsize::new(0),
        })
    }

    pub(crate) fn restarts(&self) -> usize {
        self.restarts.load(Ordering::SeqCst)
    }

    /// The answer `join_by_invite` gives from now on (`Ok` by default).
    pub(crate) fn set_join_result(&self, r: Result<(), ActionError>) {
        *self.join_result.lock().unwrap() = r;
    }

    /// How many times `join_by_invite` was called.
    pub(crate) fn join_calls(&self) -> usize {
        self.joins.load(Ordering::SeqCst)
    }

    /// The reply `members` and member actions give from now on.
    pub(crate) fn set_members(&self, r: Result<MembersReply, MemberError>) {
        *self.members.lock().unwrap() = r;
    }

    fn members_reply(&self) -> MemberFuture {
        self.member_calls.fetch_add(1, Ordering::SeqCst);
        let reply = match &*self.members.lock().unwrap() {
            Ok(r) => Ok(r.clone()),
            Err(MemberError::Action(e)) => Err(MemberError::Action(e.clone())),
            Err(MemberError::Refused { code, message }) => Err(MemberError::Refused {
                code,
                message: message.clone(),
            }),
        };
        Box::pin(async move { reply })
    }
}

impl NodeControl for SnapNode {
    fn snapshot(&self) -> NodeSnapshot {
        self.snapshot.lock().unwrap().clone()
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
    fn join_by_invite(
        &self,
        _head: crate::invite::InviteHead,
        _peers: Vec<crate::transport::net::PeerAddr>,
        _save: super::SaveKey,
    ) -> Result<(), ActionError> {
        self.joins.fetch_add(1, Ordering::SeqCst);
        self.join_result.lock().unwrap().clone()
    }
    fn stop(&self) {
        self.stops.fetch_add(1, Ordering::SeqCst);
    }
    fn restart(&self) -> Result<(), ActionError> {
        self.restarts.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn invite_join(&self) -> Option<InviteJoinView> {
        self.invite_join.lock().unwrap().clone()
    }
    fn members(&self) -> MemberFuture {
        self.members_reply()
    }
    fn member_action(&self, _act: MemberActionReq) -> MemberFuture {
        self.members_reply()
    }
}

/// A slot handle over `node` and `config`.
pub(crate) fn slot_handle(id: &str, node: Arc<dyn NodeControl>, config: ConfigState) -> SlotHandle {
    SlotHandle {
        id: NetSlotId::parse(id).expect("a valid slot id"),
        node,
        config: Arc::new(Mutex::new(config)),
    }
}

/// A config state with no file and, if given, the network key the slot works with.
pub(crate) fn config_with_key(key: Option<&str>) -> ConfigState {
    use crate::config::config_file::{prepare, CliOverrides, ConfigSource};
    let run = prepare(&ConfigSource::Disabled, &CliOverrides::default(), false, 0)
        .expect("prepare without a file");
    let mut state = ConfigState::from_run(&run);
    state.current_network_key = key.map(str::to_string);
    state
}

/// The real router and `App` over `host`; loopback access mode, no files, no sockets.
pub(crate) fn test_app(host: Arc<dyn HostControl>) -> (Router, Arc<App>) {
    use crate::config::config_file::ListenAccess;
    let auth = AuthState::new("mesh_test".to_string(), None, ListenAccess::Localhost);
    let app = Arc::new(App {
        shared: Arc::new(WebShared::new(PORT, TOKEN.to_string(), auth)),
        host,
        token_dir: None,
        stopping: std::sync::atomic::AtomicBool::new(false),
        pending_invites: Default::default(),
    });
    (super::routes::router(app.clone()), app)
}

/// One authorised request from the loopback client; the body is parsed as JSON (`Null` when it is not).
pub(crate) async fn send(
    router: &Router,
    method: Method,
    path: &str,
    body: &str,
) -> (StatusCode, Value) {
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
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
