//! Tests for `put_keeping`: protecting the page from self-lockout with a `web.ifaces` value.

use super::*;
use crate::web_gui::http::ConnInfo;

fn conn(peer: &str, local: &str) -> ConnInfo {
    ConnInfo {
        client_addr: peer.parse().unwrap(),
        local: local.parse().unwrap(),
    }
}

fn lan_conn() -> ConnInfo {
    conn("192.0.2.20:50000", "192.0.2.10:7070")
}

fn put_ifaces(state: &mut ConfigState, value: Value, c: Option<ConnInfo>) -> (StatusCode, Value) {
    let body = put_body(
        state.file_report.rev,
        serde_json::json!({"web": {"ifaces": value}}),
    );
    body_json(state.put_keeping(&body, &StubNode::new(), c).0)
}

#[test]
fn unknown_iface_from_lan_is_422_and_file_untouched() {
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let before = std::fs::read(dir.join("config.json")).unwrap();
    let (status, v) = put_ifaces(
        &mut state,
        serde_json::json!(["no-such0"]),
        Some(lan_conn()),
    );
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{v:?}");
    let fields = v["fields"].as_array().unwrap();
    assert!(fields.iter().any(|f| f["path"] == "web.ifaces"
        && f["message"]
            .as_str()
            .unwrap()
            .starts_with("this list would close the page for you")));
    assert_eq!(std::fs::read(dir.join("config.json")).unwrap(), before);
    cleanup(&dir);
}

#[test]
fn same_put_from_loopback_is_ok() {
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let loopback = conn("127.0.0.1:50000", "127.0.0.1:7070");
    let (status, v) = put_ifaces(&mut state, serde_json::json!(["no-such0"]), Some(loopback));
    assert_eq!(status, StatusCode::OK, "{v:?}");
    cleanup(&dir);
}

#[test]
fn same_put_without_conn_is_ok() {
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let (status, v) = put_ifaces(&mut state, serde_json::json!(["no-such0"]), None);
    assert_eq!(status, StatusCode::OK, "{v:?}");
    cleanup(&dir);
}

#[test]
fn star_from_lan_is_ok() {
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let (status, v) = put_ifaces(&mut state, serde_json::json!(["*"]), Some(lan_conn()));
    assert_eq!(status, StatusCode::OK, "{v:?}");
    cleanup(&dir);
}
