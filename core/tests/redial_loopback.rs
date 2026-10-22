//! Integration test: a node brings up a connection to a known but not connected member by itself —
//! without DHT and the address cache (`NodeConfig::for_loopback_test`: the `mem` network, `peers_cache_path: None`).
//! With the old code a node has no way to learn a member's address again from anywhere — after `Down`
//! the dialing is not repeated. The node sample and `wait_until` are from `core/tests/invite_loopback.rs`.
//!
//! What it does: (1) A learns B's address (`PeerAddr::new(id)`) while the A-B line is cut
//!   (`MemNet::cut`: no paths, new ones forbidden); after a few seconds the line is repaired
//!   (`heal`) — A brings up the connection by itself through a redial, without a repeated `introduce`; the test
//!   looks in the log for the loud line of attempt 1 as confirmation that exactly the redial worked, and
//!   checks that they didn't become members before `heal`; (2) A and B are already members — three repeated
//!   `introduce` calls with the same address must produce neither new dials nor retry lines.
//! What it doesn't do: it doesn't check DHT and the address cache; it doesn't check the retry pause by a number — only
//!   the fact and the absence of a dial. A member restart with the same key is checked by a live run
//!   (`reconnects_after_peer_restarts_on_same_port` went away together with the OS port: `mem` has no port).

use std::time::Duration;

use meshtown_core::members::network_key::NetworkKey;
use meshtown_core::node::output;
use meshtown_core::node::{self, NodeConfig};
use meshtown_core::transport::net::mem::process_net;
use meshtown_core::transport::net::PeerAddr;
use tokio::time::{sleep, Instant};

/// Polls the condition every 50 ms until it becomes true or the limit expires — the same
/// technique as in `invite_loopback.rs`.
async fn wait_until(limit: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + limit;
    loop {
        if cond() {
            return true;
        }
        if Instant::now() >= deadline {
            return cond();
        }
        sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn redials_peer_that_came_up_later() {
    let key = NetworkKey::generate().unwrap();

    // given: A and B are in one `mem` network, the line between them is cut
    let a = node::start_node(NodeConfig::for_loopback_test(key.clone()))
        .await
        .unwrap();
    let b = node::start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap();
    process_net().cut(a.id, b.id);

    // when: A learns B's address (`introduce`), and the dialing runs into the cut
    let before = output::log_summary(0).last_seq;
    a.introduce(PeerAddr::new(b.id)).await;
    sleep(Duration::from_secs(3)).await;
    assert!(
        !a.members.is_member(b.id),
        "members can't be made over a cut line"
    );

    // when: the line is repaired
    process_net().heal(a.id, b.id);

    // then: A brings up the connection with B by itself without a repeated `introduce`, by means of a redial
    let reconnected = wait_until(Duration::from_secs(40), || {
        a.members.is_member(b.id) && b.members.is_member(a.id)
    })
    .await;
    assert!(reconnected, "A must bring up the connection with B by itself after `heal` within 40 s without a new introduce");

    // and: the connection came through a redial — the log has the loud line of `PeerDialer::on_dial_failed` on
    // attempt 1 for B; the process-wide ring is shared with neighboring tests, but the page
    // limit is taken with a margin
    let short_b = output::short_peer_id(&b.id);
    let page = output::log_page(before, 5000);
    let redial_attempted = page
        .lines
        .iter()
        .any(|l| l.text.contains(&short_b) && l.text.contains("attempt 1"));
    assert!(
        redial_attempted,
        "the log must have a redial line (attempt 1) for B: {:?}",
        page.lines
    );

    a.shutdown().await;
    b.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_redial_while_connected() {
    let key = NetworkKey::generate().unwrap();

    // given: A and B are members of each other
    let a = node::start_node(NodeConfig::for_loopback_test(key.clone()))
        .await
        .unwrap();
    let b = node::start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap();
    a.introduce(PeerAddr::new(b.id)).await;
    let connected = wait_until(Duration::from_secs(20), || {
        a.members.is_member(b.id) && b.members.is_member(a.id)
    })
    .await;
    assert!(
        connected,
        "A and B did not become members of each other within 20 s"
    );
    let before = output::log_summary(0).last_seq;

    // when: A finds the address of the already connected B three times, then a 7 s pause
    for _ in 0..3 {
        a.introduce(PeerAddr::new(b.id)).await;
    }
    sleep(Duration::from_secs(7)).await;

    // then: after the mark — neither a new handshake with B, nor redial lines about it
    let page = output::log_page(before, 2000);
    let handshake_marker = format!("handshake with {}", output::short_peer_id(&b.id));
    let short_b = output::short_peer_id(&b.id);
    assert!(
        page.lines
            .iter()
            .all(|l| !l.text.contains(&handshake_marker)),
        "an already connected member must not be redialed: {:?}",
        page.lines
    );
    assert!(
        page.lines.iter().all(|l| {
            !(l.text.contains(&short_b) && (l.text.contains("retry") || l.text.contains("redial")))
        }),
        "there must be no redial lines for the already connected B: {:?}",
        page.lines
    );

    a.shutdown().await;
    b.shutdown().await;
}
