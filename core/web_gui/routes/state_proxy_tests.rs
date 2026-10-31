//! `/api/state` against the listener: `restart_pending` for the `socks.*` fields follows the face the node reports.

use std::sync::Arc;

use axum::http::{Method, StatusCode};
use serde_json::{json, Value};

use crate::config::config_file::ListenAccess;
use crate::proxy::ProxyFace;
use crate::web_gui::test_host::{config_with_key, send, slot_handle, test_app, FakeHost, SnapNode};

#[tokio::test]
async fn state_restart_pending_follows_the_proxy_listener() {
    // given: the file says localhost while the listener serves localnet on the file's port
    let node = SnapNode::new(Default::default());
    let mut config = config_with_key(None);
    config.file_settings.process.socks_access = ListenAccess::Localhost;
    config.boot_file_settings.process = config.file_settings.process.clone();
    let port = config.file_settings.process.socks_port;
    node.set_applied_proxy(Some(face(port, ListenAccess::Localnet)));
    let slot = slot_handle("main", node.clone(), config);
    let router = test_app(FakeHost::new(vec![slot])).0;
    set_boot(&node, "aaaaaaaa");
    assert_eq!(pending(&router).await, json!(["socks.access"]));
    // when: another lap starts with the listener unchanged
    set_boot(&node, "bbbbbbbb");
    // then: the file is still pending against the listener
    assert_eq!(pending(&router).await, json!(["socks.access"]));
    // and when: the listener takes the file's access, a lap later
    node.set_applied_proxy(Some(face(port, ListenAccess::Localhost)));
    set_boot(&node, "cccccccc");
    // then: nothing is pending
    assert_eq!(pending(&router).await, json!([]));
}

/// A node with a main slot whose proxy face differs from its file, and an added slot with its own file.
fn main_and_added() -> (axum::Router, Arc<SnapNode>) {
    let main_node = SnapNode::new(Default::default());
    let mut main_cfg = config_with_key(None);
    main_cfg.file_settings.process.socks_access = ListenAccess::Localhost;
    main_cfg.boot_file_settings.process = main_cfg.file_settings.process.clone();
    let port = main_cfg.file_settings.process.socks_port;
    main_node.set_applied_proxy(Some(face(port, ListenAccess::Localhost)));
    let added_node = SnapNode::new(Default::default());
    added_node.set_applied_proxy(Some(face(port, ListenAccess::Localnet)));
    let main = slot_handle("main", main_node, main_cfg);
    let added = slot_handle("ab2c3d", added_node.clone(), config_with_key(Some("k")));
    let router = test_app(FakeHost::new(vec![main, added])).0;
    (router, added_node)
}

#[tokio::test]
async fn added_slot_state_never_lists_socks_as_pending() {
    // given: the proxy face differs from the primary file; the added slot's config was never read
    let (router, _) = main_and_added();
    // when: its state is asked first
    let (status, body) = send(&router, Method::GET, "/api/net/ab2c3d/state", "").await;
    // then: no socks path is pending
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let pending = body["restart_pending"].as_array().unwrap();
    assert!(
        pending
            .iter()
            .all(|p| !p.as_str().unwrap().starts_with("socks.")),
        "pending: {pending:?}"
    );
}

#[tokio::test]
async fn overview_lists_no_restart_slot_for_the_overlaid_process_block() {
    let (router, _) = main_and_added();
    let (status, body) = send(&router, Method::GET, "/api/overview", "").await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["restart_slots"], json!([]));
}

async fn pending(router: &axum::Router) -> Value {
    let (status, body) = send(router, Method::GET, "/api/state", "").await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    body["restart_pending"].clone()
}

fn set_boot(node: &Arc<SnapNode>, id: &str) {
    node.snapshot.lock().unwrap().boot_id = id.to_string();
}

fn face(port: u16, access: ListenAccess) -> ProxyFace {
    ProxyFace {
        addr: Some(([127, 0, 0, 1], port).into()),
        access,
        ifaces: Default::default(),
    }
}
