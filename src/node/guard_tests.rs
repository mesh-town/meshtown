use super::*;

const LIM: MemoryLimitsKib = MemoryLimitsKib { warn_kib: 400 * 1024, exit_kib: 900 * 1024 };
const FRESH: Duration = Duration::MAX;

#[test]
fn quiet_below_warn() {
    assert_eq!(decide_memory_action(100 * 1024, 90 * 1024, LIM, FRESH), MemoryAction::Quiet);
}

#[test]
fn first_crossing_reports() {
    assert_eq!(decide_memory_action(410 * 1024, 390 * 1024, LIM, FRESH), MemoryAction::Report);
    // a crossing — at once, even if the summary was a second ago
    assert_eq!(decide_memory_action(410 * 1024, 390 * 1024, LIM, Duration::from_secs(1)), MemoryAction::Report);
}

#[test]
fn repeat_waits_ten_seconds() {
    assert_eq!(decide_memory_action(500 * 1024, 490 * 1024, LIM, Duration::from_secs(9)), MemoryAction::Quiet);
    assert_eq!(decide_memory_action(500 * 1024, 490 * 1024, LIM, Duration::from_secs(10)), MemoryAction::Report);
}

#[test]
fn exit_at_second_limit() {
    assert_eq!(decide_memory_action(900 * 1024, 500 * 1024, LIM, Duration::from_secs(1)), MemoryAction::Exit);
}

#[test]
fn no_data_is_quiet() {
    assert_eq!(decide_memory_action(0, 500 * 1024, LIM, FRESH), MemoryAction::Quiet);
}

#[test]
fn zero_limits_are_off() {
    let no_warn = MemoryLimitsKib { warn_kib: 0, exit_kib: 900 * 1024 };
    assert_eq!(decide_memory_action(500 * 1024, 100, no_warn, FRESH), MemoryAction::Quiet);
    assert_eq!(decide_memory_action(950 * 1024, 100, no_warn, FRESH), MemoryAction::Exit);
    let no_exit = MemoryLimitsKib { warn_kib: 400 * 1024, exit_kib: 0 };
    assert_eq!(decide_memory_action(5000 * 1024, 100, no_exit, FRESH), MemoryAction::Report);
    let off = MemoryLimitsKib::from_mib(0, 0);
    assert_eq!(decide_memory_action(5000 * 1024, 100, off, FRESH), MemoryAction::Quiet);
}

#[test]
fn guard_lines_read_as_in_log() {
    let head = header_text(450_000, 400_000, 12, 8, 2, "connections 3, paths 5");
    assert_eq!(
        head,
        "memory 450000 KiB (+50000 KiB/s), runtime tasks 12, runtime threads 8, checks 2, connections 3, paths 5"
    );
    assert_eq!(exit_text(950_000, LIM), "memory 950000 KiB ≥ 900 MiB — exit (code 3)");
}

#[test]
fn stalled_line_names_hanging_time() {
    assert_eq!(
        stalled_text(450_000, 400_000, 7),
        "memory 450000 KiB (+50000 KiB/s), the previous summary didn't finish in 7 s"
    );
}

#[test]
fn dump_first_report_then_every_200_mib() {
    assert!(dump_due(410 * 1024, 0));
    assert!(!dump_due(609 * 1024, 410 * 1024));
    assert!(!dump_due(609 * 1024 + 1023, 410 * 1024));
    assert!(dump_due(610 * 1024, 410 * 1024));
}

#[test]
fn dump_path_has_secs_and_mib() {
    let p = dump_path(std::path::Path::new("/d"), 1700000000, 410 * 1024 + 5);
    assert_eq!(p, std::path::PathBuf::from("/d/heap-1700000000-410.prof"));
}

/// Guard lines for a peer's paths (`mem` has no paths — no lines).
mod peer_lines_of_conn {
    use crate::net::contract::{secret, must_finish, SEED_A};
    use crate::net::{MemNet, PeerConn, PeerId, SplitTransport, Transport, AUTH};
    use crate::output::{log_page, log_summary, short_peer_id};
    use std::sync::Arc;

    use super::peer_lines;

    /// Seed of a far mem peer: no other test takes it, so a short id in the shared
    /// log unambiguously points at the lines of this very test.
    const QUIET_PEER_SEED: u8 = 0x54;

    #[tokio::test]
    async fn peer_lines_on_mem_prints_nothing() {
        // given: a mem connection — it has no paths — and a peer that is in no other tests
        let mem = mem_conn().await;
        let before = log_summary(0).last_seq;
        // when: the guard prints lines for the peer's paths
        peer_lines(&*mem.conn);
        // then: there isn't a single guard line about this peer in the log
        let lines = guard_lines_about(before, mem.conn.peer());
        assert!(lines.is_empty(), "printed for mem: {lines:?}");
        mem.close().await;
    }

    // ---- helpers ----

    /// Texts of the `[guard]` lines printed after `after` (the log's `last_seq` before the call) that
    /// start with "peer {short}: ".
    fn guard_lines_about(after: u64, peer: PeerId) -> Vec<String> {
        let prefix = format!("peer {}: ", short_peer_id(&peer));
        let page = log_page(after, 10_000);
        let ours = page
            .lines
            .into_iter()
            .filter(|line| line.tag == "guard" && line.text.starts_with(&prefix));
        ours.map(|line| line.text).collect()
    }

    /// A mem connection and its two transports (closed at the end of the test).
    struct MemSide {
        conn: Arc<dyn PeerConn>,
        a: Arc<SplitTransport>,
        b: Arc<SplitTransport>,
    }

    impl MemSide {
        async fn close(self) {
            must_finish(self.a.close()).await;
            must_finish(self.b.close()).await;
        }
    }

    async fn mem_conn() -> MemSide {
        let net = MemNet::new();
        let a = net.transport(&secret(SEED_A));
        let b = net.transport(&secret(QUIET_PEER_SEED));
        b.serve(&[AUTH]);
        let (dialed, accepted) = tokio::join!(must_finish(a.connect(&b.local_addr(), AUTH)), must_finish(b.accept()));
        let _ = accepted.expect("accept mem closed");
        let conn = dialed.expect("connect mem gave no connection");
        MemSide { conn, a, b }
    }
}

/// Thread handle: stopping without a node and without `Members`.
mod handle {
    use super::*;
    use crate::network_key::NetworkKey;
    use crate::node::config::NodeConfig;
    use crate::node::start::start_node;

    const DEADLINE: Duration = Duration::from_secs(3);

    /// Polling every 20 ms until the deadline: "join with a timeout".
    fn wait_finished(h: &GuardHandle, limit: Duration) -> bool {
        let end = Instant::now() + limit;
        while Instant::now() < end {
            if h.is_finished() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        h.is_finished()
    }

    #[test]
    fn stop_ends_thread_before_next_tick() {
        let h = start_thread(|s| while s.sleep_tick_unless_stopped() {}).unwrap();
        assert!(!h.is_finished(), "the thread must be alive until stop");
        let t = Instant::now();
        h.stop();
        assert!(wait_finished(&h, DEADLINE), "the thread didn't exit after stop");
        assert!(t.elapsed() < Duration::from_millis(900), "exit took longer than a tick: {:?}", t.elapsed());
    }

    #[test]
    fn stop_twice_and_from_clone_is_harmless() {
        let h = start_thread(|s| while s.sleep_tick_unless_stopped() {}).unwrap();
        let copy = h.clone();
        copy.stop();
        assert!(wait_finished(&h, DEADLINE));
        h.stop();
        copy.stop();
    }

    #[test]
    fn dropping_every_handle_ends_thread() {
        let h = start_thread(|s| while s.sleep_tick_unless_stopped() {}).unwrap();
        let token = h.alive.clone();
        drop(h);
        let end = Instant::now() + DEADLINE;
        while token.strong_count() > 0 && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(token.strong_count(), 0, "the thread didn't exit after the handles were dropped");
    }

    async fn node_with_guard(warn_mib: u64) -> crate::node::RunningNode {
        let mut cfg = NodeConfig::for_loopback_test(NetworkKey::generate().unwrap());
        cfg.memory.warn_mib = warn_mib;
        start_node(cfg).await.unwrap()
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shutdown_stops_mem_guard_thread() {
        let node = node_with_guard(1_000_000).await;
        let h = node.memory_guard.clone().expect("the guard must start");
        assert!(!h.is_finished());
        node.shutdown().await;
        assert!(wait_finished(&h, DEADLINE), "the mem-guard thread is alive after shutdown");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn two_starts_in_a_row_leave_at_most_one_guard() {
        let a = node_with_guard(1_000_000).await;
        let ha = a.memory_guard.clone().unwrap();
        a.shutdown().await;
        let b = node_with_guard(1_000_000).await;
        let hb = b.memory_guard.clone().unwrap();
        assert!(wait_finished(&ha, DEADLINE), "guard A is alive");
        assert!(!hb.is_finished(), "guard B must be alive");
        b.shutdown().await;
        assert!(wait_finished(&hb, DEADLINE), "guard B is alive after shutdown");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn default_loopback_has_no_guard() {
        let node = node_with_guard(0).await;
        assert!(node.memory_guard.is_none());
        node.shutdown().await;
    }
}
