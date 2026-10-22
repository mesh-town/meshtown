use super::*;
use crate::net::facade::test_support::{pid, secret, within, Probe, Spy};
use crate::net::mem::MemNet;
use crate::net::service::EXIT;
use crate::net::{NetStack, NetStackBuilder};
use crate::output::log_page;

#[test]
fn exit_line_silent_without_traffic() {
    assert_eq!(exit_line("node", 300, 0, "p", "window 1 byte"), None);
}

#[test]
fn exit_line_lists_bytes_and_congestion() {
    let line = exit_line("node", 1500000, 1100, "mem: path", "window 12000 bytes").unwrap();
    assert!(
        line.starts_with("node: sent 1500000 bytes in 5 s; path: mem: path; "),
        "got: {line}"
    );
    assert!(line.ends_with("window 12000 bytes"), "got: {line}");
}

#[test]
fn exit_line_hyper_path_without_congestion_has_no_tail() {
    let line = exit_line("node", 10, 1, "hyper: direct UDX, rtt unknown", "").unwrap();
    assert!(line.ends_with("path: hyper: direct UDX, rtt unknown"), "got: {line}");
}

mod watch {
    use super::*;

    /// A tag for this test only: other log lines don't interfere with the "no lines" check.
    const TAG: &str = "exit-watch-closed-test";

    // NOTE: the `tokio` of this project has no `test-util` (no `time::pause`), so `start_paused` is unavailable —
    // the test runs in real time, ~5 s (one `PERIOD`).
    #[tokio::test]
    async fn stops_after_conn_close_without_lines() {
        // given: a mem connection (for `mem`, `exit_tx` gives (0, 0)), closed locally
        let (_stacks, conn) = connected().await;
        conn.close(0, b"done");

        // when: `log_link_stats_while_open` runs through a period
        let done = tokio::time::timeout(PERIOD + Duration::from_secs(5), log_link_stats_while_open(conn, pid(2), TAG));

        // then: the task finished on its own, there are no lines with its tag in the log
        done.await
            .expect("watch did not stop after the connection closed");
        let lines = log_page(0, 10_000).lines;
        let ours = lines.iter().filter(|line| line.tag == TAG).count();
        assert_eq!(ours, 0, "there should be no lines on a closed mem connection");
    }
}

/// A client (node 1) connection to an exit (node 2) on the mem network; the stacks are returned so they live to the end of the test.
async fn connected() -> ((NetStack, NetStack), Arc<dyn PeerConn>) {
    let net = MemNet::new();
    let server = NetStackBuilder::new(secret(2))
        .transport(Spy::on(&net, &secret(2)).shared())
        .serve(EXIT, Probe::new())
        .build()
        .expect("exit built");
    let client = NetStackBuilder::new(secret(1))
        .transport(Spy::on(&net, &secret(1)).shared())
        .build()
        .expect("client built");
    client.add_hints(server.invite_addrs()[0].clone());
    let conn = within("connect", client.connect(pid(2), EXIT))
        .await
        .expect("connection to the exit");
    ((client, server), conn)
}
