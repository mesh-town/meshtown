//! End-to-end test of the `exit.clients` field in `GET /api/state`: a live exit node B,
//! started by the real
//! `main.rs` wiring (`common::boot_with_exit_hosts`), returns over HTTP `exit.clients == []` while
//! there are no clients and `exit.clients == [<id_full A>]` when A holds a successful exit stream.
//!
//! What it does: B is `boot_with_exit_hosts` (its exit is on by default in the settings); A is a bare
//! node `node::start_node` with the same key: `Booted` doesn't return an `Endpoint`, and `exit::ask_exit_to_connect` can't be
//! called without it. A joins the network by B's invite from `GET /api/network/invite`. Behind B's exit is a
//! "silent" site: the client asks for the name `site.test`, which the test `exit_hosts` table on B
//! leads to the site (the literal 127.0.0.1 is not opened by the exit — address filter, code 7; the table bypasses
//! the filter and is not set from working paths, so only `boot_with_exit_hosts` sets it).
//! A's streams stay open until the end of the test.
//! What it doesn't do: doesn't check the member filter and sorting (unit tests
//! `node_ctl/snapshot/build_exit_tests.rs`) and doesn't wait for the client to be forgotten (`core/tests/exit_clients.rs`).
//! Links: plumbing — `core/tests/common/node_web.rs`; `id_full` of the node is `id.to_string()`, as in
//! `NodeCtl::snapshot`.

#[path = "common/node_web.rs"]
mod common;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use meshtown_core::exit::{self, PeerConnPool};
use meshtown_core::invite;
use meshtown_core::invite::redeem::{redeem, RedeemOpts};
use meshtown_core::members::network_key::NetworkKey;
use meshtown_core::node::{self, NodeConfig, RunningNode};
use meshtown_core::transport::net::mem::process_net;
use meshtown_core::transport::net::{BoxRecv, BoxSend};
use meshtown_core::transport::net::{NodeSecret, PeerId};
use serde_json::{json, Value};
use tokio::net::TcpListener;

use common::{api, boot_consulting_with_exit_hosts, teardown, wait_ready, wait_until, Booted};

const MEMBER_WITHIN: Duration = Duration::from_secs(20);
const LISTED_WITHIN: Duration = Duration::from_secs(10);

/// The name of the "silent" site in B's `exit_hosts` table.
const SITE_NAME: &str = "site.test";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn state_lists_exit_client_after_ok_stream() {
    // given: B is a live exit node with a "silent" site in `exit_hosts`, A is a member of the same network;
    // B has no clients yet
    let key = NetworkKey::generate().unwrap();
    let site = silent_site().await;
    let hosts = HashMap::from([(SITE_NAME.to_string(), site)]);
    let b = boot_consulting_with_exit_hosts("exit-clients-b", &key, hosts).await;
    wait_ready(&b).await;
    let (a, id_b) = join_client(&b).await;
    let id_a = a.id.to_string();
    wait_b_sees_member(&b, &id_a).await;
    then_exit_on_without_clients(&b);

    // when: A asks B to open a connection to the site and holds the stream open
    let _held = when_ask_ok(&a, id_b).await;

    // then: within ≤10 s `GET /api/state` on B returns A as the only client
    then_listed_in_state(&b, &id_a).await;

    a.shutdown().await;
    teardown(b, Duration::from_secs(5)).await;
}

/// B's exit is on, and `exit.clients` is an empty array, not a missing field.
fn then_exit_on_without_clients(b: &Booted) {
    let exit = exit_of(b);
    assert_eq!(exit["enabled"], json!(true), "B's exit is on by default");
    assert_eq!(
        exit["clients"],
        json!([]),
        "without clients `exit.clients` is an empty array, not a missing field"
    );
}

/// A asks B to open a connection to the "silent" site by the name from B's `exit_hosts` (the port is replaced by the value
/// of the table); the response must be `EXIT_CODE_OK`. The stream ends are returned to the caller: while they are
/// held, the stream on A's side is not closed.
async fn when_ask_ok(a: &RunningNode, id_b: PeerId) -> (BoxSend, BoxRecv) {
    let (send, recv, code) =
        exit::ask_exit_to_connect(&a.stack, &PeerConnPool::default(), id_b, SITE_NAME, 80)
            .await
            .expect("the request to the exit must reach a response");
    assert_eq!(
        code,
        exit::EXIT_CODE_OK,
        "B's exit must open a connection to the site"
    );
    (send, recv)
}

/// Waits until `exit.clients` in B's `/api/state` is exactly `[id_a]`.
async fn then_listed_in_state(b: &Booted, id_a: &str) {
    let ok = wait_until(LISTED_WITHIN, || exit_of(b)["clients"] == json!([id_a])).await;
    assert!(
        ok,
        "within {LISTED_WITHIN:?} A did not become a client in B's /api/state: {}",
        exit_of(b)
    );
}

/// The `exit` field from a fresh `GET /api/state` of the node.
fn exit_of(node: &Booted) -> Value {
    let (status, state) = api(node.port, &node.token, "GET", "/api/state", None);
    assert_eq!(status, 200, "/api/state must answer 200");
    state["exit"].clone()
}

/// Starts A and joins it to the network by B's invite; waits until A recognizes B
/// as a member. Returns A and B's id.
async fn join_client(b: &Booted) -> (RunningNode, PeerId) {
    let (status, body) = api(b.port, &b.token, "GET", "/api/network/invite", None);
    assert_eq!(status, 200, "B's invite must be assembled");
    let text = body["invite"].as_str().expect("invite is a string");
    let parsed = invite::parse(text).expect("B's invite must parse");
    let addr_b = parsed.peers[0].clone();
    let id_b = addr_b.id;

    let owner = NodeSecret::from_bytes(&[7; 32]);
    let transport = node::bind_ephemeral(Some(&process_net())).await.unwrap();
    let opts = RedeemOpts::for_head(&parsed.head);
    let answer = redeem(
        &parsed.head,
        &parsed.peers,
        &opts,
        &|_| {},
        transport,
        &owner,
    )
    .await
    .expect("B's invite must be redeemed");
    let mut cfg = NodeConfig::for_loopback_test(parsed.head.reply_key(&answer));
    cfg.open_list = false;
    cfg.net_root = parsed.head.root;
    cfg.reply_secret = answer;
    cfg.identity_secret = Some(owner);
    cfg.settings.exit_enabled = false;
    let a = node::start_node(cfg).await.expect("A must start");
    a.introduce(addr_b).await;
    let ok = wait_until(MEMBER_WITHIN, || a.members.is_member(id_b)).await;
    assert!(
        ok,
        "A did not recognize B as a member within {MEMBER_WITHIN:?}"
    );
    (a, id_b)
}

/// Waits until A (by full id) appears in B's `peers` with `online: true`.
async fn wait_b_sees_member(b: &Booted, id_a: &str) {
    let ok = wait_until(MEMBER_WITHIN, || {
        let (_, state) = api(b.port, &b.token, "GET", "/api/state", None);
        let peers = state["peers"].as_array().cloned().unwrap_or_default();
        peers
            .iter()
            .any(|p| p["id_full"] == json!(id_a) && p["online"] == json!(true))
    })
    .await;
    assert!(
        ok,
        "B did not see A online among peers within {MEMBER_WITHIN:?}"
    );
}

/// A "silent" site: accepts connections, holds the sockets, writes and closes nothing.
async fn silent_site() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    meshtown_core::node::output::spawn(async move {
        let mut held = Vec::new();
        while let Ok((socket, _)) = listener.accept().await {
            held.push(socket);
        }
    });
    addr
}
