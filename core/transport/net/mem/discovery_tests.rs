//! Tests of `mem` discovery (`discovery.rs`): a board by name, publishing, subscription, end of work.

use crate::transport::net::mem::{MemDiscovery, MemNet};
use crate::transport::net::{AddrHint, Discovery, NodeSecret, PeerAddr, PeerId};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};

mod reading_a_board {
    use super::*;

    #[tokio::test]
    async fn gives_what_was_published_before_run() {
        let net = MemNet::new();
        let (writer, reader) = (net.discovery("board", true), net.discovery("board", false));
        writer.publish(&addr(1, 1000)).await.unwrap();

        let (mut rx, _run) = spawn_run(&reader, 16);

        assert!(
            wait_for(&mut rx, &addr(1, 1000)).await,
            "the board snapshot did not reach out"
        );
    }

    #[tokio::test]
    async fn gives_what_was_published_after_run_started() {
        let net = MemNet::new();
        let (writer, reader) = (net.discovery("board", true), net.discovery("board", false));
        let (mut rx, _run) = spawn_run(&reader, 16);
        sleep(Duration::from_millis(50)).await; // the subscription and the snapshot are already behind us

        writer.publish(&addr(1, 1000)).await.unwrap();

        assert!(
            wait_for(&mut rx, &addr(1, 1000)).await,
            "the new entry did not reach out"
        );
    }

    #[tokio::test]
    async fn gives_both_the_old_and_the_new_in_one_run() {
        let net = MemNet::new();
        let (writer, reader) = (net.discovery("board", true), net.discovery("board", false));
        writer.publish(&addr(1, 1000)).await.unwrap();
        let (mut rx, _run) = spawn_run(&reader, 16);
        sleep(Duration::from_millis(50)).await;
        writer.publish(&addr(2, 2000)).await.unwrap();

        assert!(
            wait_for(&mut rx, &addr(1, 1000)).await,
            "the old entry did not arrive"
        );
        assert!(
            wait_for(&mut rx, &addr(2, 2000)).await,
            "the new entry did not arrive"
        );
    }

    #[tokio::test]
    async fn does_not_filter_the_readers_own_address() {
        let net = MemNet::new();
        let both = net.discovery("board", true);
        let (mut rx, _run) = spawn_run(&both, 16);

        both.publish(&addr(1, 1000)).await.unwrap();

        assert!(
            wait_for(&mut rx, &addr(1, 1000)).await,
            "the own address must not be filtered out"
        );
    }
}

mod a_source_that_does_not_publish {
    use super::*;

    #[tokio::test]
    async fn answers_publish_with_ok() {
        let silent = MemNet::new().discovery("board", false);

        let res = silent.publish(&addr(1, 1000)).await;

        assert!(
            res.is_ok(),
            "publish of a reading source must be Ok(()): {res:?}"
        );
    }

    #[tokio::test]
    async fn leaves_the_board_empty() {
        let net = MemNet::new();
        let silent = net.discovery("board", false);
        let probe = net.discovery("board", true);
        silent.publish(&addr(1, 1000)).await.unwrap();
        let (mut rx, _run) = spawn_run(&probe, 16);

        let got = collect_for(&mut rx, Duration::from_millis(300)).await;

        assert!(got.is_empty(), "the board must be empty, got {got:?}");
        // control: the observation window works — an entry of a real publisher is visible in it
        probe.publish(&addr(2, 2000)).await.unwrap();
        assert!(
            wait_for(&mut rx, &addr(2, 2000)).await,
            "control: the publisher's entry did not arrive"
        );
    }

    #[test]
    fn reports_it_does_not_publish() {
        let net = MemNet::new();

        assert!(!net.discovery("a", false).publishes());
        assert!(net.discovery("b", true).publishes());
    }
}

mod boards_by_name {
    use super::*;

    #[test]
    fn the_source_reports_the_name_it_was_given() {
        let d = MemNet::new().discovery("invite", true);

        assert_eq!(d.name(), "invite");
    }

    #[tokio::test]
    async fn different_names_do_not_see_each_other() {
        let net = MemNet::new();
        let (writer, other) = (net.discovery("one", true), net.discovery("two", true));
        let (mut rx_other, _run_other) = spawn_run(&other, 16);
        let (mut rx_same, _run_same) = spawn_run(&writer, 16);

        writer.publish(&addr(1, 1000)).await.unwrap();

        // control: a source with the same name sees the entry — the observation window works
        assert!(
            wait_for(&mut rx_same, &addr(1, 1000)).await,
            "control: the same name did not see the entry"
        );
        let got = collect_for(&mut rx_other, Duration::from_millis(300)).await;
        assert!(got.is_empty(), "another name saw a foreign entry: {got:?}");
    }

    #[tokio::test]
    async fn the_same_name_in_two_nets_gives_two_boards() {
        let (net1, net2) = (MemNet::new(), MemNet::new());
        let (writer, stranger) = (net1.discovery("board", true), net2.discovery("board", true));
        let (mut rx_stranger, _run) = spawn_run(&stranger, 16);

        writer.publish(&addr(1, 1000)).await.unwrap();

        let got = collect_for(&mut rx_stranger, Duration::from_millis(300)).await;
        assert!(
            got.is_empty(),
            "the board of another network saw an entry: {got:?}"
        );
    }

    #[tokio::test]
    async fn two_sources_of_one_name_share_the_board() {
        let net = MemNet::new();
        let (first, second) = (net.discovery("board", true), net.discovery("board", true));
        first.publish(&addr(1, 1000)).await.unwrap();
        second.publish(&addr(2, 2000)).await.unwrap();
        let (mut rx, _run) = spawn_run(&second, 16);

        // the order of the board snapshot is not defined — we read the whole window at once
        let got = collect_for(&mut rx, Duration::from_millis(300)).await;
        assert!(
            got.contains(&addr(1, 1000)),
            "the entry of the first source is not visible: {got:?}"
        );
        assert!(
            got.contains(&addr(2, 2000)),
            "the entry of the second source is not visible: {got:?}"
        );
    }
}

mod publishing_an_id_again {
    use super::*;

    #[tokio::test]
    async fn replaces_the_address_for_a_late_reader() {
        let net = MemNet::new();
        let (writer, reader) = (net.discovery("board", true), net.discovery("board", false));
        writer.publish(&addr(1, 1000)).await.unwrap();
        writer.publish(&addr(1, 2000)).await.unwrap(); // the same id, a new address (restart)

        let (mut rx, _run) = spawn_run(&reader, 16);

        let got = collect_for(&mut rx, Duration::from_millis(300)).await;
        assert!(
            got.contains(&addr(1, 2000)),
            "the new address is not in {got:?}"
        );
        assert!(
            !got.contains(&addr(1, 1000)),
            "the old address stayed on the board: {got:?}"
        );
    }

    #[tokio::test]
    async fn is_announced_to_a_running_reader() {
        let net = MemNet::new();
        let (writer, reader) = (net.discovery("board", true), net.discovery("board", false));
        let (mut rx, _run) = spawn_run(&reader, 16);
        sleep(Duration::from_millis(50)).await;

        writer.publish(&addr(1, 1000)).await.unwrap();
        writer.publish(&addr(1, 2000)).await.unwrap();

        assert!(
            wait_for(&mut rx, &addr(1, 2000)).await,
            "the new address was not broadcast"
        );
    }

    #[tokio::test]
    async fn keeps_the_other_ids_untouched() {
        let net = MemNet::new();
        let (writer, reader) = (net.discovery("board", true), net.discovery("board", false));
        writer.publish(&addr(1, 1000)).await.unwrap();
        writer.publish(&addr(2, 2000)).await.unwrap();
        writer.publish(&addr(1, 3000)).await.unwrap();

        let (mut rx, _run) = spawn_run(&reader, 16);

        let got = collect_for(&mut rx, Duration::from_millis(300)).await;
        assert!(
            got.contains(&addr(2, 2000)),
            "the entry of another id disappeared: {got:?}"
        );
    }
}

mod a_reader_that_fell_behind {
    use super::*;

    #[tokio::test]
    async fn still_gets_every_address_after_the_subscription_lagged() {
        let net = MemNet::new();
        let (writer, reader) = (net.discovery("lag", true), net.discovery("lag", false));
        let (mut rx, _run) = spawn_run(&reader, 1); // out has room for one: the reader stalls
        sleep(Duration::from_millis(50)).await;

        for n in 0..600 {
            writer.publish(&addr_of(peer_n(n), 1000)).await.unwrap(); // the broadcast holds 256
        }

        let seen = ids_until(&mut rx, 600).await;
        assert_eq!(
            seen.len(),
            600,
            "not all entries of the board arrived after Lagged"
        );
    }
}

mod stopping {
    use super::*;

    #[tokio::test]
    async fn run_ends_within_a_second_after_out_is_dropped() {
        let reader = MemNet::new().discovery("board", false);
        let (rx, run) = spawn_run(&reader, 16);
        sleep(Duration::from_millis(50)).await;

        drop(rx);

        assert!(
            ends_within_a_second(run).await,
            "run did not end after the receiver was dropped"
        );
    }

    #[tokio::test]
    async fn run_blocked_on_a_full_out_ends_after_out_is_dropped() {
        let net = MemNet::new();
        let (writer, reader) = (net.discovery("board", true), net.discovery("board", false));
        let (rx, run) = spawn_run(&reader, 1);
        for n in 1..=3 {
            writer.publish(&addr(n, 1000)).await.unwrap(); // out has room for one, nobody reads
        }
        sleep(Duration::from_millis(50)).await;

        drop(rx);

        assert!(
            ends_within_a_second(run).await,
            "run hung on a full out after the receiver was dropped"
        );
    }
}

// ---- helpers ----

fn peer_n(n: u16) -> PeerId {
    let mut seed = [0xAB; 32];
    seed[..2].copy_from_slice(&n.to_le_bytes());
    NodeSecret::from_bytes(&seed).peer_id()
}

fn addr_of(id: PeerId, port: u16) -> PeerAddr {
    PeerAddr {
        id,
        hints: vec![AddrHint::Ip(format!("127.0.0.1:{port}").parse().unwrap())],
    }
}

/// Address of node `n` (its secret is `[n; 32]`) with one hint at port `port`.
fn addr(n: u8, port: u16) -> PeerAddr {
    addr_of(NodeSecret::from_bytes(&[n; 32]).peer_id(), port)
}

/// Starts the source's `run_source`; the `out` receiver has capacity `cap`.
fn spawn_run(d: &Arc<MemDiscovery>, cap: usize) -> (mpsc::Receiver<PeerAddr>, JoinHandle<()>) {
    let (tx, rx) = mpsc::channel(cap);
    (rx, tokio::spawn(d.clone().run_source(tx)))
}

/// Waits in `rx` for the address `want` for up to 2 s (repeats and foreign addresses are allowed).
async fn wait_for(rx: &mut mpsc::Receiver<PeerAddr>, want: &PeerAddr) -> bool {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while let Ok(Some(got)) = tokio::time::timeout_at(deadline, rx.recv()).await {
        if got == *want {
            return true;
        }
    }
    false
}

/// Everything that arrived in `rx` within `span`.
async fn collect_for(rx: &mut mpsc::Receiver<PeerAddr>, span: Duration) -> Vec<PeerAddr> {
    let deadline = tokio::time::Instant::now() + span;
    let mut got = Vec::new();
    while let Ok(Some(addr)) = tokio::time::timeout_at(deadline, rx.recv()).await {
        got.push(addr);
    }
    got
}

/// The set of distinct ids from `rx`, until there are `want` of them (up to 5 s).
async fn ids_until(rx: &mut mpsc::Receiver<PeerAddr>, want: usize) -> HashSet<PeerId> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut seen = HashSet::new();
    while seen.len() < want {
        let Ok(Some(got)) = tokio::time::timeout_at(deadline, rx.recv()).await else {
            break;
        };
        seen.insert(got.id);
    }
    seen
}

async fn ends_within_a_second(run: JoinHandle<()>) -> bool {
    timeout(Duration::from_secs(1), run).await.is_ok()
}
