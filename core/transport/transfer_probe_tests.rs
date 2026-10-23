//! Tests for the volume probe: size choice (no network) and `send_probe`/`answer_probes`/`send_probes_while_open` on a `mem` pair.
//! No live network is needed here: the probe talks to `PeerConn`.

use super::*;
use crate::transport::net::contract::{must_finish, secret, SEED_A, SEED_B};
use crate::transport::net::NodeSecret;
use crate::transport::net::{MemNet, PeerConn, SplitTransport, Transport, AUTH};
use portable_atomic::AtomicU64;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::task::JoinHandle;

#[test]
fn to_exit_cycles_through_three_sizes() {
    let sizes: Vec<usize> = (0..6).map(|t| probe_size_bytes(t, true, false)).collect();
    assert_eq!(sizes, [16384, 262144, 1048576, 16384, 262144, 1048576]);
}

#[test]
fn non_exit_never_gets_one_mib() {
    for turn in 0..9 {
        assert_ne!(probe_size_bytes(turn, false, false), 1048576, "turn {turn}");
    }
    let sizes: Vec<usize> = (0..4).map(|t| probe_size_bytes(t, false, false)).collect();
    assert_eq!(sizes, [16384, 262144, 16384, 262144]);
}

#[test]
fn full_cycle_acts_like_exit() {
    for turn in 0..9 {
        assert_eq!(
            probe_size_bytes(turn, false, true),
            probe_size_bytes(turn, true, false)
        );
    }
    assert_eq!(probe_size_bytes(2, false, true), 1048576);
}

#[test]
fn exit_list_change_flips_to_exit() {
    let peer = NodeSecret::from_bytes(&[3u8; 32]).peer_id();
    let tx = tokio::sync::watch::Sender::new(Vec::<PeerId>::new());
    let rx = tx.subscribe();
    assert!(!rx.borrow().contains(&peer));
    assert_ne!(
        probe_size_bytes(2, rx.borrow().contains(&peer), false),
        1048576
    );
    tx.send_replace(vec![peer]);
    assert_eq!(
        probe_size_bytes(2, rx.borrow().contains(&peer), false),
        1048576
    );
}

mod size_within_limit {
    use super::*;

    #[tokio::test]
    async fn mem_probe_round_trip() {
        // given: `answer_probes` is running on B
        let fx = setup().await;
        // when: A sends a 16 KiB probe
        let sent = must_finish(send_probe(&*fx.a, 16 * 1024)).await;
        // then: an acknowledgment of the same length arrived
        assert!(
            sent.is_ok(),
            "16 KiB probe without acknowledgment: {sent:?}"
        );
    }

    #[tokio::test]
    async fn mem_probe_at_limit_acked() {
        // given: `answer_probes` is running on B
        let fx = setup().await;
        // when: A sends exactly the maximum size
        let sent = must_finish(send_probe(&*fx.a, MAX_PROBE_BYTES)).await;
        // then: the limit is inclusive — acknowledged
        assert!(sent.is_ok(), "probe of {MAX_PROBE_BYTES} bytes: {sent:?}");
    }
}

mod size_over_limit {
    use super::*;

    #[tokio::test]
    async fn mem_probe_over_limit_refused() {
        // given: `answer_probes` is running on B
        let fx = setup().await;
        // when: A declares one byte over the limit
        let sent = must_finish(send_probe(&*fx.a, MAX_PROBE_BYTES + 1)).await;
        // then: no acknowledgment — B's stream is closed, not acknowledged: `read_exact` sees an early end
        let err = sent.expect_err("a probe over the limit must not be acknowledged");
        assert!(
            format!("{err:#}").contains("early eof"),
            "expected a refusal without acknowledgment (early eof), got: {err:#}"
        );
    }

    #[tokio::test]
    async fn mem_probe_after_refusal_still_served() {
        // given: B has already refused one probe over the limit
        let fx = setup().await;
        let refused = must_finish(send_probe(&*fx.a, MAX_PROBE_BYTES + 1)).await;
        assert!(refused.is_err(), "a probe over the limit was acknowledged");
        // when: A sends an ordinary probe on the same connection
        let sent = must_finish(send_probe(&*fx.a, 16 * 1024)).await;
        // then: `answer_probes` is alive and acknowledges
        assert!(
            sent.is_ok(),
            "probe not acknowledged after the refusal: {sent:?}"
        );
    }
}

mod serve_loop {
    use super::*;

    #[tokio::test]
    async fn mem_serve_stops_on_close() {
        // given: `answer_probes` is running on B
        let fx = setup().await;
        // when: A closes the connection
        fx.a.close(0, b"done");
        // then: B's `accept` returned an error, and `answer_probes` exited on its own
        let ended = must_finish(fx.serving).await;
        assert!(ended.is_ok(), "the serve task failed: {ended:?}");
    }
}

mod drive_on_closed_conn {
    use super::*;

    #[tokio::test]
    async fn drive_stops_on_closed() {
        // given: the connection is closed before the first turn; the interval is huge — only the close
        // check, not the end of the sleep, can make it exit earlier
        let fx = setup().await;
        let (probe, ok) = settings();
        fx.a.close(0, b"done");
        // when: we start `send_probes_while_open` without delay
        let driving =
            crate::node::output::spawn(send_probes_while_open(fx.a.clone(), fx.a.peer(), probe));
        // then: it exited on its own and sent no probe
        let ended = must_finish(driving).await;
        assert!(ended.is_ok(), "the drive task failed: {ended:?}");
        assert_eq!(
            ok.load(Ordering::Relaxed),
            0,
            "a probe went over a closed connection"
        );
    }
}

mod drive_on_live_conn {
    use super::*;

    #[tokio::test]
    async fn mem_drive_counts_successful_probe() {
        // given: the connection is alive, `answer_probes` is running on B
        let fx = setup().await;
        let (probe, ok) = settings();
        // when: `send_probes_while_open` makes its first turn
        let driving =
            crate::node::output::spawn(send_probes_while_open(fx.a.clone(), fx.a.peer(), probe));
        // then: the counter of acknowledged probes grew to one
        must_finish(wait_ok_probes(&ok, 1)).await;
        driving.abort();
    }
}

// ---- helpers ----

/// A pair of `mem` connections A→B: A sends the probes, `answer_probes` runs on the accepting side B.
struct Fixture {
    a: Arc<dyn PeerConn>,
    serving: JoinHandle<()>,
    /// The transports keep the `mem` network alive until the end of the test.
    _nodes: [Arc<SplitTransport>; 2],
}

async fn setup() -> Fixture {
    let net = MemNet::new();
    let a = net.transport(&secret(SEED_A));
    let b = net.transport(&secret(SEED_B));
    b.serve(&[AUTH]);
    let (dialed, accepted) = tokio::join!(
        must_finish(a.connect(&b.local_addr(), AUTH)),
        must_finish(b.accept())
    );
    let dialed = dialed.expect("connect A→B gave no connection");
    let (_, accepted) = accepted.expect("accept B is closed, yet a connection was awaited");
    // assumption: `answer_probes(conn: Arc<dyn PeerConn>, peer: PeerId)` — the "volume probe" contract
    let serving = crate::node::output::spawn(answer_probes(accepted.clone(), accepted.peer()));
    Fixture {
        a: dialed,
        serving,
        _nodes: [a, b],
    }
}

/// Settings for `send_probes_while_open`: no delay before the first turn, the interval is an hour (a second turn does not
/// happen in the test); the counter of acknowledged probes is returned separately.
// assumption: the `ProbeSettings` fields are the same as before (the probe change does not touch them)
fn settings() -> (ProbeSettings, Arc<AtomicUsize>) {
    let ok = Arc::new(AtomicUsize::new(0));
    let probe = ProbeSettings {
        first_delay: Duration::ZERO,
        interval_secs: Arc::new(AtomicU64::new(3600)),
        ok_probe_count: ok.clone(),
        exits: watch::channel(Vec::new()).1,
        full_cycle: false,
    };
    (probe, ok)
}

/// Waits until the counter of acknowledged probes reaches `at_least`; the caller sets the deadline (`must_finish`).
async fn wait_ok_probes(ok: &AtomicUsize, at_least: usize) {
    while ok.load(Ordering::Relaxed) < at_least {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
