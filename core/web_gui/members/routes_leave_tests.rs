//! "Leave network": the node is told first, then the host retires the slot; a broken slot is only retired.

use std::sync::Arc;

use axum::http::{Method, StatusCode};
use serde_json::{json, Value};

use crate::config::config_file::NetSlotId;
use crate::web_gui::members::dto::{Counts, MembersReply};
use crate::web_gui::test_host::{
    config_with_key, send, slot_handle, test_app, FakeHost, HostCall, SnapNode,
};
use crate::web_gui::{ActionError, HostError, MemberError, NodeSnapshot, PeerInfo};

const CONFIRM: &str = r#"{"confirm":true}"#;

fn reply(changed: Option<bool>) -> MembersReply {
    MembersReply {
        me: "me".to_string(),
        me_excluded: false,
        me_pending: false,
        need: 1,
        me_admission: None,
        counts: Counts {
            members: 1,
            voting: 1,
            excluded: 0,
            unlisted: 0,
            pending: 0,
        },
        members: Vec::new(),
        changed,
    }
}

fn snapshot_with_online(online: usize) -> NodeSnapshot {
    let me = PeerInfo {
        is_self: true,
        online: true,
        ..PeerInfo::default()
    };
    let neighbour = PeerInfo {
        online: true,
        ..PeerInfo::default()
    };
    let mut peers = vec![me];
    peers.extend(std::iter::repeat_n(neighbour, online));
    NodeSnapshot {
        peers,
        ..NodeSnapshot::default()
    }
}

struct Fixture {
    router: axum::Router,
    host: Arc<FakeHost>,
    node: Arc<SnapNode>,
}

/// A host with the primary slot and `ab2c3d`; the slot under test is `ab2c3d`.
fn fixture(key: Option<&str>, online: usize, answer: Result<MembersReply, MemberError>) -> Fixture {
    let node = SnapNode::new(snapshot_with_online(online));
    *node.members.lock().unwrap() = answer;
    let main = slot_handle(
        "main",
        SnapNode::new(Default::default()),
        config_with_key(None),
    );
    let slot = slot_handle("ab2c3d", node.clone(), config_with_key(key));
    let host = FakeHost::new(vec![main, slot]);
    let router = test_app(host.clone()).0;
    Fixture { router, host, node }
}

fn ab2c3d() -> NetSlotId {
    NetSlotId::parse("ab2c3d").unwrap()
}

async fn leave(f: &Fixture, body: &str) -> (StatusCode, Value) {
    send(
        &f.router,
        Method::POST,
        "/api/net/ab2c3d/members/leave",
        body,
    )
    .await
}

#[tokio::test]
async fn without_confirm_nothing_is_called() {
    // given
    let f = fixture(Some("k"), 1, Ok(reply(Some(true))));
    // when
    let (status, body) = leave(&f, "{}").await;
    // then
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "bad_request");
    assert_eq!(
        f.node
            .member_calls
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    assert!(f.host.calls().is_empty());
}

#[tokio::test]
async fn a_slot_with_a_network_announces_then_is_retired() {
    for (online, announced) in [(0, false), (1, true)] {
        // given
        let f = fixture(Some("k"), online, Ok(reply(Some(true))));
        // when
        let (status, body) = leave(&f, CONFIRM).await;
        // then
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["left"], true);
        assert_eq!(body["announced"], announced, "online {online}");
        assert!(body["nets"].is_array());
        assert_eq!(
            f.node
                .member_calls
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );
        assert_eq!(f.host.calls(), vec![HostCall::Retire(ab2c3d(), announced)]);
    }
}

#[tokio::test]
async fn a_refusal_of_the_node_keeps_the_slot() {
    for (err, status, code) in [
        (
            ActionError::Busy("busy now".to_string()),
            StatusCode::CONFLICT,
            "busy",
        ),
        (
            ActionError::Internal("disk".to_string()),
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
        ),
    ] {
        // given
        let f = fixture(Some("k"), 1, Err(MemberError::Action(err)));
        // when
        let (got, body) = leave(&f, CONFIRM).await;
        // then
        assert_eq!(got, status);
        assert_eq!(body["error"], code);
        assert!(f.host.calls().is_empty(), "the slot was retired");
    }
}

#[tokio::test]
async fn already_excluded_is_retired_with_left_false() {
    // given
    let f = fixture(Some("k"), 0, Ok(reply(Some(false))));
    // when
    let (status, body) = leave(&f, CONFIRM).await;
    // then
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["left"], false);
    assert_eq!(f.host.calls(), vec![HostCall::Retire(ab2c3d(), false)]);
}

#[tokio::test]
async fn a_slot_without_a_network_skips_the_node() {
    // given
    let f = fixture(None, 1, Ok(reply(Some(true))));
    // when
    let (status, body) = leave(&f, CONFIRM).await;
    // then
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        (body["left"].clone(), body["announced"].clone()),
        (json!(false), json!(false))
    );
    assert_eq!(
        f.node
            .member_calls
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    assert_eq!(f.host.calls(), vec![HostCall::Retire(ab2c3d(), false)]);
}

#[tokio::test]
async fn the_primary_slot_with_a_key_from_a_flag_is_busy() {
    // given
    let node = SnapNode::new(snapshot_with_online(1));
    let mut config = config_with_key(Some("k"));
    config.cli_override_paths = vec!["network.key"];
    let main = slot_handle("main", node.clone(), config);
    let host = FakeHost::new(vec![main]);
    let router = test_app(host.clone()).0;
    // when
    let (status, body) = send(&router, Method::POST, "/api/members/leave", CONFIRM).await;
    // then
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"], "busy");
    assert_eq!(
        node.member_calls.load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    assert!(host.calls().is_empty());
}

#[tokio::test]
async fn a_host_refusal_is_answered() {
    // given
    let f = fixture(None, 0, Ok(reply(None)));
    f.host
        .set_retire(Err(HostError::Busy("stopping".to_string())));
    // when
    let (status, body) = leave(&f, CONFIRM).await;
    // then
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"], "busy");
}

#[tokio::test]
async fn a_broken_slot_is_retired_without_the_node() {
    // given
    let f = fixture(Some("k"), 1, Ok(reply(Some(true))));
    let broken = NetSlotId::parse("zz2c3d").unwrap();
    f.host.set_broken(vec![broken.clone()]);
    // when
    let (status, body) = send(
        &f.router,
        Method::POST,
        "/api/net/zz2c3d/members/leave",
        CONFIRM,
    )
    .await;
    let (state, state_body) = send(&f.router, Method::GET, "/api/net/zz2c3d/state", "").await;
    // then
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (body["left"].clone(), body["announced"].clone()),
        (json!(false), json!(false))
    );
    assert_eq!(f.host.calls(), vec![HostCall::Retire(broken, false)]);
    assert_eq!(
        f.node
            .member_calls
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    assert_eq!(state, StatusCode::NOT_FOUND);
    assert_eq!(state_body["error"], "unknown_net");
}
