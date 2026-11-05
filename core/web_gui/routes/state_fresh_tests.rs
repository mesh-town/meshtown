//! `/api/state` waits for a fresh snapshot after an idle page, at most about two seconds.

use std::sync::Arc;
use std::time::Duration;

use axum::http::{Method, StatusCode};
use tokio::time::Instant;

use crate::web_gui::test_host::{config_with_key, send, slot_handle, test_app, FakeHost, SnapNode};

fn router_with(age_ms: u64) -> (axum::Router, Arc<SnapNode>) {
    let node = SnapNode::new(Default::default());
    node.snapshot.lock().unwrap().snapshot_age_ms = age_ms;
    let slot = slot_handle("main", node.clone(), config_with_key(None));
    (test_app(FakeHost::new(vec![slot])).0, node)
}

#[tokio::test(start_paused = true)]
async fn state_waits_for_a_fresh_snapshot_after_idle() {
    // given: the snapshot is 6 s old and a collection lands 300 ms later
    let (router, node) = router_with(6_000);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        node.snapshot.lock().unwrap().snapshot_age_ms = 0;
    });
    // when: the page asks
    let (status, body) = send(&router, Method::GET, "/api/state", "").await;
    // then: the answer carries the fresh snapshot
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["snapshot_age_ms"], 0);
}

#[tokio::test(start_paused = true)]
async fn state_returns_the_old_snapshot_after_two_seconds() {
    // given: the snapshot never gets fresh
    let (router, _node) = router_with(6_000);
    let started = Instant::now();
    // when: the page asks
    let (status, body) = send(&router, Method::GET, "/api/state", "").await;
    // then: the old snapshot comes back after about 2 s
    let waited = started.elapsed();
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["snapshot_age_ms"], 6_000);
    assert!(
        waited >= Duration::from_millis(1_900) && waited <= Duration::from_millis(2_500),
        "{waited:?}"
    );
}
