//! Integration tests of the node page listener, the group "invite and joining a network":
//! the invite route without a network (404 at a fake node), parsing a `key`/`invite` string in
//! `/api/network/join` — a bad word, a real invite, a damaged invite.
//! What it doesn't do: it doesn't change `src/web.rs` and `core/web/*.rs` — it only calls their public interface.
//! The token is never printed or put into the text of `assert!`/`assert_eq!`: the same rule is kept
//! for the network key — it is not a token, but it is the same shared secret of the network.
//! Links: the node page listener module, which these tests cover,
//! `docs/WEB-API.md`; the harness is `core/tests/common/web_listener.rs` (`#[path]`, shared with
//! `core/tests/web_listener_lifecycle.rs`, `_protocol.rs`, `_config_api.rs`).

#[path = "common/web_listener.rs"]
mod common;

use std::net::SocketAddr;

use meshtown_core::config::config_file::{self, LoadedConfigFile, NetSettings, Settings};
use meshtown_core::invite::{self, InviteHead, InviteSecret};
use meshtown_core::members::network_key::NetworkKey;
use meshtown_core::ports_sharing::{Audience, Published};
use meshtown_core::transport::addr_order::PrimaryIps;
use meshtown_core::transport::net::{AddrHint, NodeSecret, PeerAddr};
use meshtown_core::transport::net_mainline::unix_now;
use meshtown_core::web_gui::DEFAULT_DRAIN;
use serde_json::{json, Value};

use common::{api, cleanup, run_config, scratch, start_web};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn network_invite_route_default_not_found() {
    let dir = scratch("invite-404");
    let run = run_config(&dir);
    let (handle, _node) = start_web(&dir, &run).await;
    let port = handle.shared.port();
    let token = handle.shared.token();

    let (status, body) = api(port, &token, "GET", "/api/network/invite", None);
    assert_eq!(status, 404);
    assert_eq!(body.get("error").and_then(Value::as_str), Some("not_found"));

    handle.shutdown(DEFAULT_DRAIN).await;
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn network_join_bad_word_reports_word_field() {
    let dir = scratch("badword");
    let run = run_config(&dir);
    let (handle, _node) = start_web(&dir, &run).await;
    let port = handle.shared.port();
    let token = handle.shared.token();

    let (status, body) = api(
        port,
        &token,
        "POST",
        "/api/network/join",
        Some(json!({"key": "finch-zzqq-k7m2q9x4rb8vhn3tz01g5"})),
    );
    assert_eq!(status, 422);
    assert_eq!(
        body.get("error").and_then(Value::as_str),
        Some("invalid_key")
    );
    assert_eq!(body.get("word").and_then(Value::as_str), Some("zzqq"));

    handle.shutdown(DEFAULT_DRAIN).await;
    cleanup(&dir);
}

/// The config file in `dir` holds the key of the joined `island` network.
fn assert_stored_island_key(dir: &std::path::Path) {
    let loaded = config_file::load_config_file(&dir.join("config.json"));
    match loaded {
        LoadedConfigFile::Parsed(p) => {
            let stored = p.settings.net.network_key.unwrap_or_default();
            assert!(
                stored.starts_with("island--mesh1-"),
                "the key in the file must start with \"island--mesh1-\""
            );
        }
        other => panic!("expected Loaded::Parsed, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn network_join_accepts_invite_string() {
    let dir = scratch("joininvite");
    let run = run_config(&dir);
    let (handle, node) = start_web(&dir, &run).await;
    let port = handle.shared.port();
    let token = handle.shared.token();

    let (status0, cfg0) = api(port, &token, "GET", "/api/config", None);
    assert_eq!(status0, 200);
    let rev0 = cfg0.get("rev").and_then(Value::as_u64).expect("rev0");

    let key = NetworkKey::generate().unwrap();
    let head = invite_head(&key, unix_now());
    // the node stub gives the handler a "received key" the way a real node does
    node.save_key_on_join(&head.key_text(&key));
    let invite_string = invite_string(&head);

    let (status, body) = api(
        port,
        &token,
        "POST",
        "/api/network/join",
        Some(json!({"key": invite_string})),
    );
    assert_eq!(status, 202);
    assert_eq!(body.get("label").and_then(Value::as_str), Some("island"));
    assert_eq!(
        body.get("waiting"),
        Some(&json!(true)),
        "reply: the node is waiting for network members"
    );
    assert_eq!(
        body.get("restart"),
        Some(&json!(false)),
        "reply: no restart is needed"
    );
    assert!(
        node.got_call("join_by_invite"),
        "the string must go to the node's join_by_invite"
    );
    assert!(
        !node.got_call("switch"),
        "the network is not switched at once by the string: joining goes after the key"
    );

    assert_stored_island_key(&dir);

    let (status1, cfg1) = api(port, &token, "GET", "/api/config", None);
    assert_eq!(status1, 200);
    let rev1 = cfg1.get("rev").and_then(Value::as_u64).expect("rev1");
    assert_eq!(rev1, rev0 + 1, "rev must grow by 1 after a join by invite");

    handle.shutdown(DEFAULT_DRAIN).await;
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn network_join_rejects_broken_invite() {
    let dir = scratch("brokeninvite");
    let run = run_config(&dir);
    let (handle, _node) = start_web(&dir, &run).await;
    let port = handle.shared.port();
    let token = handle.shared.token();

    let (status, body) = api(
        port,
        &token,
        "POST",
        "/api/network/join",
        Some(json!({"key": "mesh-invite2-!!!!"})),
    );
    assert_eq!(status, 422);
    assert_eq!(
        body.get("error").and_then(Value::as_str),
        Some("invalid_key")
    );
    assert!(body.get("word").is_none(), "there must be no word field");
    let message = body
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(message.contains("foreign characters"), "message: {message}");

    handle.shutdown(DEFAULT_DRAIN).await;
    cleanup(&dir);
}

// ---- network switch with port publications ----

/// A key of the form "label--words", the way the page sends it.
fn words_key(label: &str) -> String {
    let tail = meshtown_core::members::words::generate_tail().unwrap();
    meshtown_core::members::network_key::format_key_string(Some(label), &tail)
}

/// Writes into `dir/config.json` `ports.published = [8080 All "web"]` — before `run_config` — and, when
/// `with_key` is set, the key of network A.
fn write_file_with_ports(dir: &std::path::Path, with_key: bool) {
    let key_a = with_key.then(|| words_key("a"));
    let settings = Settings {
        net: NetSettings {
            network_key: key_a,
            ports_published: vec![Published {
                port: 8080,
                name: "web".to_string(),
                audience: Audience::All,
            }],
            ..NetSettings::default()
        },
        ..Settings::default()
    };
    let secret = NodeSecret::from_bytes(&[9u8; 32]);
    config_file::save_config_file(&dir.join("config.json"), None, 1, &secret, &settings)
        .expect("save");
}

fn published_in_file(dir: &std::path::Path) -> Vec<u16> {
    match config_file::load_config_file(&dir.join("config.json")) {
        LoadedConfigFile::Parsed(p) => p
            .settings
            .net
            .ports_published
            .iter()
            .map(|x| x.port)
            .collect(),
        other => panic!("expected Loaded::Parsed, got {other:?}"),
    }
}

/// Posts to a node whose file has the published port and no network key.
async fn post_network(dir_tag: &str, path: &str, body: Value) -> (u16, Value, Vec<u16>) {
    post_network_to(dir_tag, path, body, false).await
}

async fn post_network_to(
    dir_tag: &str,
    path: &str,
    body: Value,
    with_key: bool,
) -> (u16, Value, Vec<u16>) {
    let dir = scratch(dir_tag);
    write_file_with_ports(&dir, with_key);
    let run = run_config(&dir);
    let (handle, _node) = start_web(&dir, &run).await;
    let (status, resp) = api(
        handle.shared.port(),
        &handle.shared.token(),
        "POST",
        path,
        Some(body),
    );
    let ports = published_in_file(&dir);
    handle.shutdown(DEFAULT_DRAIN).await;
    cleanup(&dir);
    (status, resp, ports)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn network_create_without_keep_ports_drops_published() {
    let (status, _, ports) =
        post_network("keep-none", "/api/network/create", json!({"label": "new"})).await;
    assert_eq!(status, 200);
    assert!(ports.is_empty(), "none by default: {ports:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn network_create_with_keep_ports_keeps_published() {
    let body = json!({"label": "new", "keep_ports": true});
    let (status, _, ports) = post_network("keep-yes", "/api/network/create", body).await;
    assert_eq!(status, 200);
    assert_eq!(ports, vec![8080]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn network_join_key_with_keep_false_drops_published() {
    let key = words_key("b");
    let body = json!({"key": key, "keep_ports": false});
    let (status, _, ports) = post_network("join-no", "/api/network/join", body).await;
    assert_eq!(status, 202);
    assert!(ports.is_empty(), "{ports:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn network_join_invite_without_field_drops_published() {
    let (status, _, ports) = post_invite_network("join-invite", json!({})).await;
    assert_eq!(status, 202);
    assert!(ports.is_empty(), "{ports:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn network_join_invite_with_keep_ports_keeps_published() {
    let (status, _, ports) =
        post_invite_network("join-invite-keep", json!({"keep_ports": true})).await;
    assert_eq!(status, 202);
    assert_eq!(ports, vec![8080]);
}

/// Joining by an invite string with a file that has port 8080 published: the node stub "receives the key"
/// and calls the handler's `save`; `extra` — extra body fields (`keep_ports`).
async fn post_invite_network(dir_tag: &str, extra: Value) -> (u16, Value, Vec<u16>) {
    post_invite_network_to(dir_tag, extra, false).await
}

async fn post_invite_network_to(
    dir_tag: &str,
    extra: Value,
    with_key: bool,
) -> (u16, Value, Vec<u16>) {
    let key = NetworkKey::generate().unwrap();
    let head = invite_head(&key, unix_now());
    let mut body = json!({"key": invite_string(&head)});
    body.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    let dir = scratch(dir_tag);
    write_file_with_ports(&dir, with_key);
    let run = run_config(&dir);
    let (handle, node) = start_web(&dir, &run).await;
    node.save_key_on_join(&head.key_text(&key));
    let (status, resp) = api(
        handle.shared.port(),
        &handle.shared.token(),
        "POST",
        "/api/network/join",
        Some(body),
    );
    let ports = published_in_file(&dir);
    handle.shutdown(DEFAULT_DRAIN).await;
    cleanup(&dir);
    (status, resp, ports)
}

// ---- a slot that already has a network refuses another one and keeps its file ----

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn network_create_on_a_slot_with_a_network_is_refused() {
    let body = json!({"label": "new", "keep_ports": false});
    let (status, resp, ports) =
        post_network_to("present-create", "/api/network/create", body, true).await;
    assert_eq!(status, 409);
    assert_eq!(resp["error"], json!("network_present"));
    assert_eq!(
        ports,
        vec![8080],
        "the refused request must not touch the file"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn network_join_key_on_a_slot_with_a_network_is_refused() {
    let body = json!({"key": words_key("b"), "keep_ports": false});
    let (status, resp, ports) =
        post_network_to("present-join", "/api/network/join", body, true).await;
    assert_eq!(status, 409);
    assert_eq!(resp["error"], json!("network_present"));
    assert_eq!(
        ports,
        vec![8080],
        "the refused request must not touch the file"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn network_join_invite_of_another_network_on_a_one_network_run_is_refused() {
    let (status, resp, ports) = post_invite_network_to("present-invite", json!({}), true).await;
    assert_eq!(status, 409);
    assert_eq!(resp["error"], json!("too_many_nets"));
    assert_eq!(
        ports,
        vec![8080],
        "the refused request must not touch the file"
    );
}

// ---- joining by an invite string: refusals before the node and the time limit boundary ----

/// The refusal texts of the invite module — verbatim.
const TEXT_OLD_INVITE: &str = "This invite is of the old format (mesh-invite1-): it contained the network key, \
    such invites are no longer accepted. Ask a network member for a new invite — from the updated program.";
const TEXT_V2_INVITE: &str = "This invite is of an older format: such invites are no longer accepted. Ask a network member for a new invite — from the updated program.";
/// An invite string of format 2.
const V2_INVITE: &str = "mesh-invite2-allk27cqadd4pr6hy7d4pr6hy7d4pr6hy7d4pr6hy7d4pr6hy7d4pr6hy7d4pahxytkqmaoxlkmadavrbk35ks762pewibz2b3qxf462uyrsllycdjupob2rdiaacad7aaaadia7qd34jvig";
const TEXT_EXPIRED: &str =
    "The invite was issued more than a day ago and is no longer valid. Ask a \
    network member for a new one.";
const HOUR: u64 = 3600;

mod old_invite_string {
    use super::*;

    /// A string of an earlier format and the refusal text it gets.
    const REFUSED: [(&str, &str); 2] = [
        ("mesh-invite1-abcdefghij", TEXT_OLD_INVITE),
        (V2_INVITE, TEXT_V2_INVITE),
    ];

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn is_refused_with_its_format_text() {
        for (n, (text, want)) in REFUSED.into_iter().enumerate() {
            // given
            let web = setup(&format!("old-invite-{n}")).await;
            // when
            let (status, body) = web.when_join_with(text);
            // then
            assert_eq!(status, 422);
            assert_eq!(
                body.get("error").and_then(Value::as_str),
                Some("invalid_key")
            );
            assert_eq!(body.get("message").and_then(Value::as_str), Some(want));
            web.finish().await;
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn is_not_given_to_the_node() {
        for (n, (text, _)) in REFUSED.into_iter().enumerate() {
            // given
            let web = setup(&format!("old-invite-node-{n}")).await;
            // when
            web.when_join_with(text);
            // then
            web.then_node_got_no_join();
            web.finish().await;
        }
    }
}

mod expired_invite_string {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn is_refused_with_the_expired_text() {
        // given: issued 25 hours ago
        let web = setup("expired-invite").await;
        let text = invite_string(&invite_head(
            &NetworkKey::generate().unwrap(),
            unix_now() - 25 * HOUR,
        ));
        // when
        let (status, body) = web.when_join_with(&text);
        // then
        assert_eq!(status, 422);
        assert_eq!(
            body.get("message").and_then(Value::as_str),
            Some(TEXT_EXPIRED)
        );
        web.finish().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn is_not_given_to_the_node() {
        // given
        let web = setup("expired-invite-node").await;
        let text = invite_string(&invite_head(
            &NetworkKey::generate().unwrap(),
            unix_now() - 25 * HOUR,
        ));
        // when
        web.when_join_with(&text);
        // then
        web.then_node_got_no_join();
        web.finish().await;
    }
}

mod invite_string_made_23_hours_ago {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn is_still_accepted() {
        // given
        let web = setup("fresh-23h").await;
        let text = invite_string(&invite_head(
            &NetworkKey::generate().unwrap(),
            unix_now() - 23 * HOUR,
        ));
        // when
        let (status, _) = web.when_join_with(&text);
        // then
        assert_eq!(status, 202, "the string is valid within a day");
        assert!(web.node.got_call("join_by_invite"));
        web.finish().await;
    }
}

// ============================================================================================
// setup and helpers
// ============================================================================================

/// The listener of the page with a node stub and a settings directory.
struct Web {
    dir: std::path::PathBuf,
    handle: meshtown_core::web_gui::WebHandle,
    node: std::sync::Arc<common::TestNode>,
}

async fn setup(tag: &str) -> Web {
    let dir = scratch(tag);
    let run = run_config(&dir);
    let (handle, node) = start_web(&dir, &run).await;
    Web { dir, handle, node }
}

impl Web {
    /// `POST /api/network/join` with the entered string: (status, body).
    fn when_join_with(&self, text: &str) -> (u16, Value) {
        let (port, token) = (self.handle.shared.port(), self.handle.shared.token());
        api(
            port,
            &token,
            "POST",
            "/api/network/join",
            Some(json!({"key": text})),
        )
    }

    fn then_node_got_no_join(&self) {
        assert!(
            !self.node.got_call("join_by_invite"),
            "the refused string reached the node"
        );
        assert!(
            !self.node.got_call("switch"),
            "the refused string switched the network"
        );
    }

    async fn finish(self) {
        self.handle.shutdown(DEFAULT_DRAIN).await;
        cleanup(&self.dir);
    }
}

/// The head of an invite of network `key` with the label "island" and a one-time stub secret.
fn invite_head(key: &NetworkKey, made_unix: u64) -> InviteHead {
    InviteHead {
        secret: InviteSecret::from_bytes([0x42; 32]),
        network_id: key.subkeys().network_id,
        label: Some("island".to_string()),
        made_unix,
        kind: meshtown_core::members::network_key::NetKind::Peer,
    }
}

/// A `mesh-invite2-…` string with one member address (nobody goes into the network by it).
fn invite_string(head: &InviteHead) -> String {
    let me = PeerAddr {
        id: NodeSecret::from_bytes(&[7u8; 32]).peer_id(),
        hints: vec![AddrHint::Ip(SocketAddr::from(([10, 0, 0, 1], 4000)))],
    };
    invite::build_keyed(head, &me, &[], &PrimaryIps::NONE)
        .expect("invite::build must assemble an invite")
}
