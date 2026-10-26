//! Slot addressing of the network routes: the prefix picks the slot, `/api/…` is the primary one.

use std::path::PathBuf;
use std::sync::Arc;

use axum::http::{Method, StatusCode};
use serde_json::{json, Value};

use crate::config::config_file::{self, CliOverrides, ConfigSource, NetSlotId, PathRule};
use crate::transport::net::NodeSecret;
use crate::web_gui::test_host::{
    config_with_key, send, slot_handle, test_app, FakeHost, HostCall, SnapNode,
};
use crate::web_gui::{ConfigState, HostControl, HostError};

struct Fixture {
    router: axum::Router,
    main: Arc<SnapNode>,
    other: Arc<SnapNode>,
}

fn fixture() -> Fixture {
    let main = SnapNode::new(Default::default());
    let other = SnapNode::new(Default::default());
    let slots = vec![
        slot_handle("main", main.clone(), config_with_key(None)),
        slot_handle("ab2c3d", other.clone(), config_with_key(None)),
    ];
    let router = test_app(FakeHost::new(slots)).0;
    Fixture {
        router,
        main,
        other,
    }
}

#[tokio::test]
async fn prefixed_restart_reaches_only_that_slot() {
    // given
    let f = fixture();
    // when
    let (status, _) = send(
        &f.router,
        Method::POST,
        "/api/net/ab2c3d/actions/restart",
        "",
    )
    .await;
    // then
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(f.other.restarts(), 1, "restarts of ab2c3d");
    assert_eq!(f.main.restarts(), 0, "restarts of the primary slot");
}

#[tokio::test]
async fn unknown_slot_is_404_unknown_net() {
    // given
    let f = fixture();
    // when
    let (status, body) = send(
        &f.router,
        Method::POST,
        "/api/net/zzzzzz/actions/restart",
        "",
    )
    .await;
    let (bad, bad_body) = send(
        &f.router,
        Method::POST,
        "/api/net/not-an-id/actions/restart",
        "",
    )
    .await;
    // then
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "unknown_net");
    assert_eq!(bad, StatusCode::NOT_FOUND);
    assert_eq!(bad_body["error"], "unknown_net");
    assert_eq!(f.main.restarts() + f.other.restarts(), 0, "restarts");
}

#[tokio::test]
async fn both_addresses_of_the_primary_slot_act_the_same() {
    // given
    let f = fixture();
    // when
    let (plain, _) = send(&f.router, Method::POST, "/api/actions/restart", "").await;
    let (named, _) = send(&f.router, Method::POST, "/api/net/main/actions/restart", "").await;
    // then
    assert_eq!((plain, named), (StatusCode::ACCEPTED, StatusCode::ACCEPTED));
    assert_eq!(f.main.restarts(), 2, "restarts of the primary slot");
    assert_eq!(f.other.restarts(), 0, "restarts of ab2c3d");
}

#[tokio::test]
async fn a_broken_slot_has_no_routes_but_the_list() {
    // given
    let main = SnapNode::new(Default::default());
    let host = FakeHost::new(vec![slot_handle("main", main, config_with_key(None))]);
    host.set_broken(vec![
        crate::config::config_file::NetSlotId::parse("ab2c3d").unwrap()
    ]);
    let router = test_app(host).0;
    // when
    let (status, body) = send(&router, Method::GET, "/api/net/ab2c3d/state", "").await;
    let (_, nets) = send(&router, Method::GET, "/api/nets", "").await;
    // then
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "unknown_net");
    assert_eq!(nets["nets"][1]["status"], "broken");
}

mod proxy_exit_choice {
    use super::*;

    mod put_to_an_added_slot {
        use super::*;

        #[tokio::test]
        async fn new_route_exits_claim_the_proxy_for_that_slot() {
            // given
            let f = setup_exit_choice();
            // when
            let (status, body) = f
                .when_put(B, json!({"route": {"exits": [exit_id(1)]}}))
                .await;
            // then
            assert_eq!(status, StatusCode::OK, "body: {body}");
            assert_eq!(f.host.calls(), vec![HostCall::ClaimProxy(slot_b())]);
        }

        #[tokio::test]
        async fn emptied_route_exits_do_not_claim() {
            // given: exits were set, which claimed once
            let f = setup_exit_choice();
            f.when_put(B, json!({"route": {"exits": [exit_id(1)]}}))
                .await;
            // when
            let (status, body) = f.when_put(B, json!({"route": {"exits": []}})).await;
            // then
            assert_eq!(status, StatusCode::OK, "body: {body}");
            assert_eq!(f.host.calls().len(), 1, "calls: {:?}", f.host.calls());
        }

        #[tokio::test]
        async fn change_of_another_field_does_not_claim() {
            // given
            let f = setup_exit_choice();
            // when
            let (status, body) = f
                .when_put(B, json!({"checks": {"interval_sec": 600}}))
                .await;
            // then
            assert_eq!(status, StatusCode::OK, "body: {body}");
            assert_eq!(f.host.calls(), vec![]);
        }

        #[tokio::test]
        async fn claim_runs_after_the_slot_config_lock_is_released() {
            // given
            let f = setup_exit_choice();
            f.host.check_lock_free_of(slot_b());
            // when
            f.when_put(B, json!({"route": {"exits": [exit_id(1)]}}))
                .await;
            // then
            assert_eq!(f.host.lock_was_free(), Some(true), "lock was free at claim");
        }

        #[tokio::test]
        async fn failed_claim_still_answers_200_and_is_logged() {
            // given
            let f = setup_exit_choice();
            f.host
                .set_claim_result(Err(HostError::Busy("test claim failure".to_string())));
            let seq_before = crate::node::output::log_summary(0).last_seq;
            // when
            let (status, body) = f
                .when_put(B, json!({"route": {"exits": [exit_id(1)]}}))
                .await;
            // then
            assert_eq!(status, StatusCode::OK, "body: {body}");
            let lines = lines_after(seq_before);
            let expected = format!("exit chosen in network {B} was not applied");
            assert!(
                lines.iter().any(|l| l.contains(&expected)),
                "journal lines: {lines:?}"
            );
        }
    }

    mod put_to_the_primary_slot_naming_proxy_net {
        use super::*;

        #[tokio::test]
        async fn existing_slot_is_claimed() {
            // given
            let f = setup_exit_choice();
            // when
            let (status, body) = f.when_put("main", json!({"proxy": {"net": B}})).await;
            // then
            assert_eq!(status, StatusCode::OK, "body: {body}");
            assert_eq!(f.host.calls(), vec![HostCall::ClaimProxy(slot_b())]);
        }

        #[tokio::test]
        async fn unknown_slot_is_404_and_nothing_is_written_or_claimed() {
            // given
            let f = setup_exit_choice();
            let rev_before = f.rev_of("main");
            // when
            let (status, body) = f
                .when_put("main", json!({"proxy": {"net": "zzzzzz"}}))
                .await;
            // then
            assert_eq!(status, StatusCode::NOT_FOUND, "body: {body}");
            assert_eq!(body["error"], "unknown_net");
            assert_eq!(f.rev_of("main"), rev_before, "rev of the primary slot");
            assert_eq!(f.host.calls(), vec![], "claim calls");
        }

        #[tokio::test]
        async fn malformed_id_is_422_and_not_claimed() {
            // given
            let f = setup_exit_choice();
            // when
            let (status, body) = f.when_put("main", json!({"proxy": {"net": "x"}})).await;
            // then
            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
            assert_eq!(f.host.calls(), vec![], "claim calls");
        }

        #[tokio::test]
        async fn change_of_another_field_does_not_claim() {
            // given
            let f = setup_exit_choice();
            // when
            let (status, body) = f
                .when_put("main", json!({"checks": {"interval_sec": 600}}))
                .await;
            // then
            assert_eq!(status, StatusCode::OK, "body: {body}");
            assert_eq!(f.host.calls(), vec![]);
        }
    }
}

const B: &str = "ab2c3d";

fn slot_b() -> NetSlotId {
    NetSlotId::parse(B).unwrap()
}

fn exit_id(seed: u8) -> String {
    NodeSecret::from_bytes(&[seed; 32]).peer_id().to_string()
}

fn lines_after(seq: u64) -> Vec<String> {
    let page = crate::node::output::log_page(seq, 10_000);
    page.lines.into_iter().map(|l| l.text).collect()
}

/// The primary slot and `ab2c3d`, both over settings files of their own, behind the real router.
struct ExitChoice {
    router: axum::Router,
    host: Arc<FakeHost>,
    dir: PathBuf,
}

impl ExitChoice {
    /// A `PUT` of `config` to the slot at the current revision of its file.
    async fn when_put(&self, slot: &str, config: Value) -> (StatusCode, Value) {
        let path = match slot {
            "main" => "/api/config".to_string(),
            id => format!("/api/net/{id}/config"),
        };
        let body = json!({"rev": self.rev_of(slot), "config": config}).to_string();
        send(&self.router, Method::PUT, &path, &body).await
    }

    fn rev_of(&self, slot: &str) -> u64 {
        let handle = self.host.slot(&NetSlotId::parse(slot).unwrap()).unwrap();
        let rev = handle.config.lock().unwrap().file_report.rev;
        rev
    }
}

impl Drop for ExitChoice {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn setup_exit_choice() -> ExitChoice {
    static COUNTER: portable_atomic::AtomicU64 = portable_atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("net-routes-{}-{n}", std::process::id()));
    let main = file_slot("main", &dir.join("main"), None);
    let added = file_slot(B, &dir.join(B), Some("some-network-key"));
    let host = FakeHost::new(vec![main, added]);
    let router = test_app(host.clone()).0;
    ExitChoice { router, host, dir }
}

fn file_slot(id: &str, dir: &std::path::Path, key: Option<&str>) -> crate::web_gui::SlotHandle {
    std::fs::create_dir_all(dir).unwrap();
    let source = ConfigSource::File {
        path: dir.join("config.json"),
        may_create: true,
        rule: PathRule::Explicit,
    };
    let run = config_file::prepare(&source, &CliOverrides::default(), true, 0).unwrap();
    let mut state = ConfigState::from_run(&run);
    state.current_network_key = key.map(str::to_string);
    slot_handle(id, SnapNode::new(Default::default()), state)
}
