//! Integration tests of a real node behind the web listener (the page listener): status, stop,
//! network change, a second member, a domain — over bare TCP through `meshtown_core::*`.
//! What it doesn't do: it doesn't edit `src/web.rs`/`core/web/*.rs`/`tests/web_listener_*.rs` — it only calls
//! their public interface. It never prints the token and never puts it in the text of `assert!`/`assert_eq!`.
//! Links: the web listener (`meshtown_core::web_gui`);
//! scaffolding in `core/tests/common/node_web.rs` (`#[path]`, shared with `core/tests/node_web_bootstrap.rs`).

#[path = "common/node_web.rs"]
mod common;

use std::time::{Duration, Instant};

use meshtown_core::members::network_key::NetworkKey;
use meshtown_core::node::output;
use meshtown_core::node::{self, NodeConfig};
use meshtown_core::web_gui::DEFAULT_DRAIN;
use serde_json::{json, Value};

use common::{api, boot, cleanup, teardown, wait_ready, wait_until};

/// (a) `web::start_web_page` + `run_node_with_ctl`: `status` is not `starting` within ≤ 10 s, `boot_id` is 8 hex digits,
/// `node.identity == "persistent"` on a fresh temporary file (replaces item 10 of the battery).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn state_leaves_starting_with_persistent_identity() {
    let key = NetworkKey::generate().unwrap();
    let booted = boot("state", &key).await;
    wait_ready(&booted).await;

    let (status, v) = api(booted.port, &booted.token, "GET", "/api/state", None);
    assert_eq!(status, 200);
    let boot_id = v["boot_id"].as_str().expect("boot_id must be a string");
    assert_eq!(boot_id.len(), 8, "boot_id must be 8 digits");
    assert!(
        boot_id.chars().all(|c| c.is_ascii_hexdigit()),
        "boot_id must consist of hex digits"
    );
    assert_eq!(
        v["node"]["identity"],
        json!("persistent"),
        "the identity must persist on a fresh file"
    );
    assert_eq!(v["node"]["identity_reason"], Value::Null);

    teardown(booted, Duration::from_secs(5)).await;
}

/// (b) `POST /api/actions/stop` → `run_node_with_ctl` returned within ≤ 5 s.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_action_ends_run_with_quickly() {
    let key = NetworkKey::generate().unwrap();
    let booted = boot("stop", &key).await;
    wait_ready(&booted).await;

    let (status, body) = api(
        booted.port,
        &booted.token,
        "POST",
        "/api/actions/stop",
        None,
    );
    assert_eq!(status, 202, "stop must return 202");
    assert_eq!(body["stopping"], json!(true));

    let started = Instant::now();
    let res = tokio::time::timeout(Duration::from_secs(5), booted.task).await;
    let elapsed = started.elapsed();
    assert!(
        elapsed <= Duration::from_secs(5),
        "run_with didn't finish within 5 s: {elapsed:?}"
    );
    assert!(
        matches!(res, Ok(Ok(Ok(())))),
        "run_with must return Ok(()) within the allotted time"
    );

    booted.handle.shutdown(DEFAULT_DRAIN).await;
    cleanup(&booted.dir);
}

/// (c) `switch_network` (through `POST /api/network/join` with a temporary config file) → a new
/// `boot_id`, `log.last_seq` didn't decrease.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn switch_network_gets_new_boot_id_without_shrinking_log() {
    let key1 = NetworkKey::generate().unwrap();
    let key2 = NetworkKey::generate().unwrap();
    let booted = boot("switch", &key1).await;
    wait_ready(&booted).await;

    let (_, before) = api(booted.port, &booted.token, "GET", "/api/state", None);
    let boot_before = before["boot_id"].as_str().unwrap().to_string();
    let last_seq_before = before["log"]["last_seq"].as_u64().unwrap();

    let (status, body) = api(
        booted.port,
        &booted.token,
        "POST",
        "/api/network/join",
        Some(json!({"key": key2.encode()})),
    );
    assert_eq!(status, 202, "join must return 202");
    assert_eq!(body["restart"], json!(true));

    let ok = wait_until(Duration::from_secs(10), || {
        let (_, v) = api(booted.port, &booted.token, "GET", "/api/state", None);
        v["boot_id"]
            .as_str()
            .map(|s| s != boot_before)
            .unwrap_or(false)
            && v["node"]["status"] != json!("starting")
    })
    .await;
    assert!(
        ok,
        "the node's new cycle didn't come up with a new boot_id within 10 s"
    );

    let (_, after) = api(booted.port, &booted.token, "GET", "/api/state", None);
    let last_seq_after = after["log"]["last_seq"].as_u64().unwrap();
    assert!(
        last_seq_after >= last_seq_before,
        "log.last_seq must not decrease on a network change: was {last_seq_before}, now {last_seq_after}"
    );

    teardown(booted, Duration::from_secs(5)).await;
}

/// (d) two nodes in one network: the first one's `peers` contains the second with `online: true`. The first is
/// the full wiring of the page listener under the same flags as the other scenarios; the second is a bare
/// `NodeConfig::for_loopback_test` (as in `core/tests/loopback.rs`), introduced through the real
/// page action `POST /api/peer-addresses` (`node_ctl::peer_lines`/`parse_peer_line`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn peers_show_second_node_online() {
    let key = NetworkKey::generate().unwrap();
    let booted = boot("peers-a", &key).await;
    wait_ready(&booted).await;

    let b = node::start_node(NodeConfig::for_loopback_test(key.clone()))
        .await
        .expect("node b must come up");
    // there is no port on `mem`: the IP hint can be anything, the node is found by key (KeyedMem)
    let line = format!("{}@127.0.0.1:9", b.id);
    let short_b = output::short_peer_id(&b.id);

    let (status, body) = api(
        booted.port,
        &booted.token,
        "POST",
        "/api/peer-addresses",
        Some(json!({"text": line})),
    );
    assert_eq!(status, 200, "adding a member's address must succeed");
    assert_eq!(body["added"], json!(1));

    let ok = wait_until(Duration::from_secs(10), || {
        let (_, v) = api(booted.port, &booted.token, "GET", "/api/state", None);
        v["peers"]
            .as_array()
            .map(|peers| {
                peers
                    .iter()
                    .any(|p| p["id"] == json!(short_b) && p["online"] == json!(true))
            })
            .unwrap_or(false)
    })
    .await;
    assert!(
        ok,
        "the second node didn't appear in peers with online:true within 10 s"
    );

    b.shutdown().await;
    teardown(booted, Duration::from_secs(5)).await;
}

/// (e) `POST /api/domains` → the domain in `domains[]`, polled for up to 5 s.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn add_domain_shows_up_in_state() {
    let key = NetworkKey::generate().unwrap();
    let booted = boot("domain", &key).await;
    wait_ready(&booted).await;

    let (status, body) = api(
        booted.port,
        &booted.token,
        "POST",
        "/api/domains",
        Some(json!({"domain": "figma.com"})),
    );
    assert_eq!(status, 200, "adding a domain must succeed");
    assert_eq!(body["domain"], json!("figma.com"));

    let ok = wait_until(Duration::from_secs(5), || {
        let (_, v) = api(booted.port, &booted.token, "GET", "/api/state", None);
        v["domains"]
            .as_array()
            .map(|ds| ds.iter().any(|d| d["name"] == json!("figma.com")))
            .unwrap_or(false)
    })
    .await;
    assert!(ok, "the domain didn't appear in domains[] within 5 s");

    teardown(booted, Duration::from_secs(5)).await;
}

/// (f) `POST /api/actions/restart` — a real restart in the same process: the config file
/// is reread (`socks.enabled`, turned off by editing the file on disk), `boot_id` is new, `run_node_with_ctl`
/// didn't finish. `socks.enabled` is what gets checked: there is no CLI flag
/// for it in `boot`, while `socks.port` is overridden by the flag `--socks-port 0` and the file doesn't change it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restart_action_rereads_config_file() {
    // given
    let key = NetworkKey::generate().unwrap();
    let booted = boot("restart", &key).await;
    wait_ready(&booted).await;
    let (_, before) = api(booted.port, &booted.token, "GET", "/api/state", None);
    let boot_before = before["boot_id"].as_str().unwrap().to_string();
    assert_eq!(
        before["socks"]["enabled"],
        json!(true),
        "SOCKS must be on before the scenario"
    );
    // `PUT /api/config` won't work: `boot` sets `--web-port 0`, and the validator rejects (422) the merged document with
    // `web.port = 0`. So we edit the config file on disk: the restart
    // must reread exactly that.
    let file = booted.dir.join("config.json");
    let mut doc: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    doc["config"]["socks"]["enabled"] = json!(false);
    std::fs::write(&file, serde_json::to_vec_pretty(&doc).unwrap()).unwrap();

    // when
    let (status, body) = api(
        booted.port,
        &booted.token,
        "POST",
        "/api/actions/restart",
        None,
    );

    // then
    assert_eq!(status, 202, "restart must return 202");
    assert_eq!(body["restart"], json!(true));
    let ok = wait_until(Duration::from_secs(20), || {
        let (_, v) = api(booted.port, &booted.token, "GET", "/api/state", None);
        v["boot_id"].as_str().is_some_and(|s| s != boot_before)
            && v["node"]["status"] != json!("starting")
    })
    .await;
    assert!(ok, "the node didn't restart with a new boot_id within 20 s");
    let (_, after) = api(booted.port, &booted.token, "GET", "/api/state", None);
    assert_eq!(
        after["socks"]["enabled"],
        json!(false),
        "after the restart SOCKS must be off according to the file"
    );
    assert!(!booted.task.is_finished(), "run_with must not finish");

    teardown(booted, Duration::from_secs(5)).await;
}
