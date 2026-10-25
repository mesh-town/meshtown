//! Tests for `helpers.rs`: the middleware that rejects mutations while the node is stopping.

use std::sync::atomic::Ordering;

use axum::http::{Method, StatusCode};

use crate::web_gui::test_host::{send, test_app, FakeHost};

#[tokio::test]
async fn mutations_while_stopping_are_503_stopping() {
    // given
    let (router, app) = test_app(FakeHost::new(vec![]));
    app.stopping.store(true, Ordering::SeqCst);

    // when
    let (status, body) = send(&router, Method::POST, "/api/nets", "").await;

    // then
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"], "stopping");
    assert_eq!(body["message"], "Mesh Town is stopping");
}
