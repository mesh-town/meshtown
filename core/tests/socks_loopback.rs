//! Integration tests of the exit choice: the SOCKS5 server, manual `route.exits` choice
//! and an exit through a network member.
//!
//! What it does: runs a real connection through node A's SOCKS5, node B's exit and a local
//! HTTP server; checks the exit's refusal when it is off, serving any domain and any
//! port (the domain and port restrictions are lifted), manual exit choice via `route.exits` (static at start and
//! a live change without a restart) and the direct path with an empty list.
//! What it does not do: does not go to the internet, does not start TLS (the live run does that via `curl`),
//! does not check two hops in a row — there are none in the node, does not check the switch to the next
//! candidate by the exit's reply code (the decision is final).
//! Links: checks `meshtown::socks5`, `meshtown::route`, `meshtown::exit` and
//! `meshtown::proxy` through `meshtown::node`.
//!
//! This file was not part of the original test skeleton — it
//! broke the build on the old API `route::choose_for`/`Route::Peer`/`set_route_all_traffic`
//! (rewritten against the new route design). The old
//! `two_nodes_exit_serves_blocked_domain` and `all_traffic_sends_outside_domain_through_peer`
//! checked two different mechanisms (domain blocking and the "all traffic" flag), which in the new
//! model are one and the same manual `route.exits` list; they were renamed and split into "the list is
//! set statically at start" (`chosen_exit_carries_socks5_traffic`) and "the list changes with a live
//! knob" (`route_exits_is_live_and_clears_back_to_direct`), `direct_path_when_own_check_is_ok` was
//! renamed to `direct_path_when_no_exits_configured` — the path no longer depends on the result of
//! its own domain check, only on whether `route.exits` is empty.
//!
//! The helpers (`DOMAIN`, `serve`, `introduce`, `wait_member`, `wait_exit_route`,
//! `pair`, `pair_with_static_exit`) moved to `core/tests/common/socks_pair.rs` — shared with
//! `core/tests/socks_exit_reset.rs`.

#[path = "common/socks_pair.rs"]
mod common;

use std::time::Duration;

use meshtown::exit::{self, PeerConnPool};
use meshtown::http_min::{self, HttpLimits, HttpGetRequest};
use meshtown::network_key::NetworkKey;
use meshtown::node::{self, NodeConfig};
use meshtown::route::Route;
use meshtown::socks5;

use common::{pair, pair_with_static_exit, serve, wait_exit_route, DOMAIN};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn chosen_exit_carries_socks5_traffic() {
    // given: A is started with `route.exits = [b]` in the config (not a live edit).
    let site = serve(4).await;
    let (a, b) = pair_with_static_exit(site).await;

    // when: the route converges.
    let route = wait_exit_route(&a, Duration::from_secs(20)).await;
    assert_eq!(
        route,
        Route::Exits(vec![b.id]),
        "the route must lead through the chosen exit"
    );

    // then: a request through SOCKS5 goes through B and brings the whole body.
    let proxy = a.socks_addr.expect("the SOCKS5 server must be up");
    let mut stream = socks5::client_connect(proxy, DOMAIN, 80).await.unwrap();
    let response = http_min::http_get(
        &mut stream,
        HttpGetRequest {
            host: DOMAIN,
            path: "/",
            accept: "*/*",
            user_agent: "meshtown-test",
        },
        HttpLimits::default(),
    )
    .await
    .unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(response.body.len(), 4 * 1024);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn exit_serves_domain_outside_list() {
    let site = serve(1).await;
    let (a, b) = pair(site, true).await;
    let pool = PeerConnPool::default();
    let (_send, _recv, code) = exit::ask_exit_to_connect(&a.stack, &pool, b.id, "outside.test", 80)
        .await
        .unwrap();
    assert_eq!(code, exit::EXIT_CODE_OK);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn exit_serves_port_other_than_80_443() {
    let site = serve(1).await;
    let (a, b) = pair(site, true).await;
    let pool = PeerConnPool::default();
    let (_send, _recv, code) = exit::ask_exit_to_connect(&a.stack, &pool, b.id, DOMAIN, 8080)
        .await
        .unwrap();
    assert_eq!(code, exit::EXIT_CODE_OK);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn exit_refuses_when_disabled() {
    let site = serve(1).await;
    let (a, b) = pair(site, false).await;
    let pool = PeerConnPool::default();
    let (_send, _recv, code) = exit::ask_exit_to_connect(&a.stack, &pool, b.id, DOMAIN, 80)
        .await
        .unwrap();
    assert_eq!(code, exit::EXIT_CODE_REFUSED);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn direct_path_when_no_exits_configured() {
    // given: a node with defaults — `route.exits` is empty.
    let key = NetworkKey::generate().unwrap();
    let node = node::start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap();

    // when/then: the path is direct at once, without waiting for sync — the empty list decides by itself in `choose_route`.
    assert_eq!(
        node.route.choose_now().await.unwrap(),
        Route::Direct,
        "route.exits is empty by default — the path is direct"
    );
}

/// The live knob: changing `route.exits` without restarting the node — both turning on the path through an exit
/// and returning to the direct path after the list is cleared.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn route_exits_is_live_and_clears_back_to_direct() {
    let site = serve(4).await;
    let (a, b) = pair(site, true).await;

    // when: turn B on with a live edit, without restarting A.
    a.set_route_exits(vec![b.id]);
    let route = wait_exit_route(&a, Duration::from_secs(20)).await;
    assert_eq!(route, Route::Exits(vec![b.id]));

    // then: the SOCKS5 request goes through B.
    let proxy = a.socks_addr.expect("the SOCKS5 server must be up");
    let mut stream = socks5::client_connect(proxy, "outside.test", 80)
        .await
        .unwrap();
    let response = http_min::http_get(
        &mut stream,
        HttpGetRequest {
            host: "outside.test",
            path: "/",
            accept: "*/*",
            user_agent: "meshtown-test",
        },
        HttpLimits::default(),
    )
    .await
    .unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(response.body.len(), 4 * 1024);

    // when: the list is cleared with the same live knob.
    a.set_route_exits(vec![]);

    // then: the path is direct again, without restarting the node.
    assert_eq!(
        a.route.choose_now().await.unwrap(),
        Route::Direct,
        "an empty list after clearing — the path is direct, without a restart"
    );
}
