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
