//! Integration tests of a real node behind the web listener: a node without a network key,
//! first start and an invite — over bare TCP through `meshtown_core::*`.
//! What it doesn't do: it doesn't edit `src/web.rs`/`core/web/*.rs`/`tests/web_listener_*.rs` — it only calls
//! their public interface. It never prints the token and never puts it in the text of `assert!`/`assert_eq!`.
//! Links: the web listener (`meshtown_core::web_gui`);
//! scaffolding in `core/tests/common/node_web.rs` (`#[path]`, shared with `core/tests/node_web_actions.rs`).

#[path = "common/node_web.rs"]
mod common;

use std::time::Duration;

use meshtown_core::members::network_key::NetworkKey;
use serde_json::{json, Value};

use common::{api, boot, boot_without_key, teardown, wait_ready, wait_until, Booted};

/// (1) A node without a key: right after coming up `node.status == "offline"`, `network == null` —
/// the value is read directly from `phase` in `snapshot()`, without waiting.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn state_without_key_shows_offline_and_no_network() {
    let booted = boot_without_key("nonet").await;

    let (status, v) = api(booted.port, &booted.token, "GET", "/api/state", None);
    assert_eq!(status, 200);
    assert_eq!(v["node"]["status"], json!("offline"));
    assert_eq!(v["network"], Value::Null);

    teardown(booted, Duration::from_secs(3)).await;
}

/// (2) `POST /api/actions/stop` for a node without a key: `run_node_awaiting_key` returns within ≤ 3 s
/// (tighter than the 5 s of `stop_action_ends_run_with_quickly` — no node is built before `Switch`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_ends_run_after_key_quickly() {
    let booted = boot_without_key("stop-wait").await;
    teardown(booted, Duration::from_secs(3)).await;
}

/// (3) A node without a key: initial state `offline`, then `POST /api/network/join` with an ordinary
/// (not an invite) key → 202, `restart: true`, and within ≤ 10 s the status leaves `offline` and
/// `network` stops being `null`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn join_leaves_offline_state_without_key() {
    let booted = boot_without_key("join-wait").await;

    let (_, v) = api(booted.port, &booted.token, "GET", "/api/state", None);
    assert_eq!(v["node"]["status"], json!("offline"));

    let key = NetworkKey::generate().unwrap();
    let (status, body) = api(
        booted.port,
        &booted.token,
        "POST",
        "/api/network/join",
        Some(json!({"key": key.encode()})),
    );
    assert_eq!(status, 202, "join must return 202");
    assert_eq!(body["restart"], json!(true));

    let ok = wait_until(Duration::from_secs(10), || {
        let (_, v) = api(booted.port, &booted.token, "GET", "/api/state", None);
        v["node"]["status"] != json!("offline") && v["network"] != Value::Null
    })
    .await;
    assert!(ok, "the node didn't leave offline after join within 10 s");

    teardown(booted, Duration::from_secs(5)).await;
}

/// (4) Node A (with a key) hands out an invite through `GET /api/network/invite`; node B (without a key)
/// joins with it through `POST /api/network/join` → appears among B's `peers` with `online: true`
/// and with the exit flag announced by A (a foreign row gets the same `exit` as A's own row
/// from A's `/api/state`), while B's own file gets A's key without a label
/// (`key_with_label(None, &key) == key.encode()`). A's address in the invite that B receives is
/// a loopback one (`loopback_invite`): host addresses are unreachable when the default route fails.
/// Fetches A's invite (`GET /api/network/invite`) and its own row from `peers`.
/// Returns (invite, A's short id, A's exit flag).
fn create_invite_and_self_info(a: &Booted) -> (String, String, Value) {
    let (status, body) = api(a.port, &a.token, "GET", "/api/network/invite", None);
    assert_eq!(status, 200, "the invite must be built");
    let invite = body["invite"]
        .as_str()
        .expect("invite must be a string")
        .to_string();
    assert!(
        invite.starts_with("mesh-invite2-"),
        "the invite must start with mesh-invite2-"
    );
    let invite = loopback_invite(&invite);

    let (_, state_a) = api(a.port, &a.token, "GET", "/api/state", None);
    let self_a = state_a["peers"]
        .as_array()
        .expect("peers at A must be an array")
        .iter()
        .find(|p| p["self"] == json!(true))
        .expect("A's own row must be in peers");
    let short_a = self_a["id"]
        .as_str()
        .expect("A's own row must have an id")
        .to_string();
    let exit_a = self_a["exit"].clone();

    (invite, short_a, exit_a)
}

/// On `mem` A's invite already carries only the member key (no host addresses), the node is found by
/// key in the process-wide network, so the string goes as is.
fn loopback_invite(invite: &str) -> String {
    invite.to_string()
}

/// `POST /api/network/join` with an invite string: (status, response body).
fn post_invite(node: &Booted, invite: &str) -> (u16, Value) {
    api(
        node.port,
        &node.token,
        "POST",
        "/api/network/join",
        Some(json!({"key": invite})),
    )
}

/// B joins with A's invite (202 "waiting for members") and waits until A appears among its `peers` with
/// `online: true` and the same exit flag A had.
async fn join_via_invite_and_wait_online(b: &Booted, invite: &str, short_a: &str, exit_a: &Value) {
    let (status, body) = post_invite(b, invite);
    assert_eq!(status, 202, "join with an invite must return 202");
    assert_eq!(
        body["waiting"],
        json!(true),
        "response to an invite string: the node waits for members instead of joining immediately"
    );
    assert_eq!(
        body["restart"],
        json!(false),
        "no restart is needed when joining with an invite"
    );

    let ok = wait_until(Duration::from_secs(20), || {
        let (_, v) = api(b.port, &b.token, "GET", "/api/state", None);
        v["peers"]
            .as_array()
            .map(|peers| {
                peers.iter().any(|p| {
                    p["id"] == json!(short_a) && p["online"] == json!(true) && p["exit"] == *exit_a
                })
            })
            .unwrap_or(false)
    })
    .await;
    assert!(
        ok,
        "node A didn't appear in B's peers with online:true and A's exit flag within 20 s"
    );
}

/// Checks that B sees A's exit flag and its own, and that B's config file holds A's key
/// without a label.
fn assert_peer_a_seen_by_b_and_key_saved(
    b: &Booted,
    short_a: &str,
    exit_a: &Value,
    key: &NetworkKey,
) {
    let (_, state_b) = api(b.port, &b.token, "GET", "/api/state", None);
    let peers_b = state_b["peers"]
        .as_array()
        .expect("peers at B must be an array");
    let peer_a_entry = peers_b
        .iter()
        .find(|p| p["id"] == json!(short_a))
        .expect("A's row must be in B's peers");
    assert!(exit_a.is_boolean(), "A's own row must publish exit");
    assert_eq!(peer_a_entry["exit"], *exit_a, "A's exit flag must reach B");
    let self_entry = peers_b
        .iter()
        .find(|p| p["self"] == json!(true))
        .expect("B's own row must be in B's peers");
    assert!(
        self_entry["exit"].is_boolean(),
        "B's own row must publish exit"
    );

    let raw = std::fs::read(b.dir.join("config.json")).expect("B's config file must be readable");
    let v: Value = serde_json::from_slice(&raw).expect("B's config file must be JSON");
    assert_eq!(
        v["config"]["network"]["key"],
        json!(key.encode()),
        "the key in B's file must have no label"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invite_join_shows_peer_and_saves_mesh1_key() {
    let key = NetworkKey::generate().unwrap();
    let a = boot("invite-a", &key).await;
    wait_ready(&a).await;

    let (invite, short_a, exit_a) = create_invite_and_self_info(&a);

    let b = boot_without_key("invite-b").await;
    join_via_invite_and_wait_online(&b, &invite, &short_a, &exit_a).await;
    assert_peer_a_seen_by_b_and_key_saved(&b, &short_a, &exit_a, &key);

    teardown(b, Duration::from_secs(5)).await;
    teardown(a, Duration::from_secs(5)).await;
}

/// (5) A node without a key: `GET /api/network/invite` → 409 `{"error": "busy"}` (`ActionError::Busy`
/// from `invite_pending()`, the `Phase::NoNetwork` branch).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invite_busy_without_network() {
    let booted = boot_without_key("invite-busy").await;

    let (status, body) = api(
        booted.port,
        &booted.token,
        "GET",
        "/api/network/invite",
        None,
    );
    assert_eq!(status, 409);
    assert_eq!(body["error"], json!("busy"));

    teardown(booted, Duration::from_secs(3)).await;
}

// ---- single use: the same string at a third node ----

mod invite_string_spent_by_another_node {
    use super::*;

    const TEXT_USED: &str = "Another node has already joined with this invite — it is single-use. \
        Ask a network member for a new one.";

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn third_node_is_refused_with_the_used_text_and_stays_offline() {
        // given: A issued a string, B joined with it and is visible at A — the string is spent
        let spent = setup_spent_string().await;

        // when: the same string at node C
        let (status, body) = post_invite(&spent.c, &spent.invite);

        // then: the response is the same "waiting for members", while C's state shows a refusal
        assert_eq!(
            status, 202,
            "the repeated string is accepted for waiting, the refusal comes in the state"
        );
        assert_eq!(body["waiting"], json!(true));
        let view = wait_refused_view(&spent.c).await;
        assert_eq!(view["message"], json!(TEXT_USED));
        let (_, state_c) = api(spent.c.port, &spent.c.token, "GET", "/api/state", None);
        assert_eq!(
            state_c["node"]["status"],
            json!("offline"),
            "the refused node got no network"
        );
        assert_eq!(state_c["network"], Value::Null);

        spent.teardown().await;
    }
}

// ---- setup and helpers ----

/// A is in the network, B joined with A's string, C is still without a network.
struct SpentString {
    a: Booted,
    b: Booted,
    c: Booted,
    invite: String,
}

async fn setup_spent_string() -> SpentString {
    let key = NetworkKey::generate().unwrap();
    let a = boot("spent-a", &key).await;
    wait_ready(&a).await;
    let (invite, short_a, exit_a) = create_invite_and_self_info(&a);
    let b = boot_without_key("spent-b").await;
    join_via_invite_and_wait_online(&b, &invite, &short_a, &exit_a).await;
    let c = boot_without_key("spent-c").await;
    SpentString { a, b, c, invite }
}

impl SpentString {
    async fn teardown(self) {
        teardown(self.c, Duration::from_secs(5)).await;
        teardown(self.b, Duration::from_secs(5)).await;
        teardown(self.a, Duration::from_secs(5)).await;
    }
}

/// Waits until `node.invite_join.state` becomes `"refused"`, then returns that object.
async fn wait_refused_view(node: &Booted) -> Value {
    let refused = wait_until(Duration::from_secs(20), || {
        let (_, v) = api(node.port, &node.token, "GET", "/api/state", None);
        v["node"]["invite_join"]["state"] == json!("refused")
    })
    .await;
    let (_, state) = api(node.port, &node.token, "GET", "/api/state", None);
    assert!(
        refused,
        "the refusal didn't appear in node.invite_join within 20 s: {}",
        state["node"]["invite_join"]
    );
    state["node"]["invite_join"].clone()
}
