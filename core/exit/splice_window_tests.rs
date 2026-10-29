//! Tests of `splice_windowed` (`splice.rs`): the cap on bytes handed to the client stream before an acknowledgement,
//! holding at the cap, both directions byte-exact, dropping the future and the other side's grace period while
//! a flush waits. The client stream is the stub of `windowed_copy_stub.rs`; the wires are those of `splice_tests.rs`.

use super::*;
use crate::exit::windowed_copy::stub::{ack_writer, pattern, AckHandle, AckMode};

/// What the site sends in the `preloaded` tests: fits the socket buffers, so the exit reads it without stalling.
const SITE_BYTES: usize = 48 * 1024;
const SMALL_WINDOW: usize = 16 * 1024;
const BIG_WINDOW: usize = 64 * 1024;
const MIB: usize = 1024 * 1024;
const PATIENCE: Duration = Duration::from_secs(5);

#[tokio::test]
async fn plain_splice_hands_everything_before_any_ack() {
    // given: a site that sent 48 KiB and closed, a client stream that never acknowledges, no window
    let (send, acks) = manual_send();
    let site_bytes = pattern(SITE_BYTES);
    let p = preloaded(send, None, &site_bytes).await;

    // when: splice runs
    // then: all of it is handed to the stream without waiting for an acknowledgement
    acks.wait_written(SITE_BYTES, PATIENCE).await;
    assert_eq!(
        acks.written(),
        site_bytes,
        "the site's bytes arrive unchanged"
    );
    p.task.abort();
}

#[tokio::test]
async fn windowed_splice_holds_at_most_the_window() {
    // given: the same site and a client stream that doesn't acknowledge, with a window of 16 KiB
    let (send, acks) = manual_send();
    let site_bytes = pattern(SITE_BYTES);
    let p = preloaded(send, Some(SMALL_WINDOW), &site_bytes).await;

    // when: splice runs without any acknowledgement
    acks.wait_written(SMALL_WINDOW, PATIENCE).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // then: it stands on the window
    assert_eq!(
        acks.written_len(),
        SMALL_WINDOW,
        "splice went past the window"
    );

    // when: the acknowledgements keep coming
    release_all(&acks, SITE_BYTES).await;

    // then: everything arrives, never more than the window at a time
    assert_eq!(
        acks.written(),
        site_bytes,
        "the site's bytes arrive unchanged"
    );
    assert!(
        acks.max_unacked() <= SMALL_WINDOW,
        "{} unacknowledged",
        acks.max_unacked()
    );
    p.task.abort();
}

#[tokio::test]
async fn windowed_splice_carries_both_directions_byte_exact() {
    // given: a windowed splice of 64 KiB over a client stream that acknowledges at once
    let mut w = setup_with(Some(BIG_WINDOW)).await;
    let (up, down) = (pattern(MIB), pattern(MIB + 1000)[1000..].to_vec());

    // when: a mebibyte goes each way and both sides close
    let (at_site, at_client) = exchange(&mut w, &up, &down).await;

    // then: both sides got exactly what the other sent, and the counts say so
    assert_eq!(at_site, up, "client to site");
    assert_eq!(at_client, down, "site to client");
    assert_eq!(finished(&mut w).await, (MIB as u64, MIB as u64));
}

#[tokio::test]
async fn dropping_splice_while_flush_waits_closes_the_site() {
    // given: a windowed splice standing on a full window, waiting for an acknowledgement that never comes
    let (send, acks) = manual_send();
    let mut p = preloaded(send, Some(SMALL_WINDOW), &pattern(SITE_BYTES)).await;
    acks.wait_written(SMALL_WINDOW, PATIENCE).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // when: the splice future is dropped
    p.task.abort();
    let _ = (&mut p.task).await;

    // then: the site's connection ends within a second (a reset: unread bytes were left in the socket)
    let mut buf = [0u8; 1];
    let ended = tokio::time::timeout(Duration::from_secs(1), p.site.read(&mut buf)).await;
    let read = ended.expect("the site's connection must end");
    let closed = matches!(&read, Ok(0))
        || matches!(&read, Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset);
    assert!(
        closed,
        "expected the end of the stream or a reset, got {read:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn half_close_grace_cuts_a_flush_that_never_settles() {
    // given: a windowed splice standing on a full window, waiting for an acknowledgement that never comes
    let (send, acks) = manual_send();
    let mut p = preloaded(send, Some(SMALL_WINDOW), &pattern(SITE_BYTES)).await;
    acks.wait_written(SMALL_WINDOW, PATIENCE).await;

    // when: the client closes its side without a byte
    p.client_out.shutdown().await.expect("client close");

    // then: after the short grace of a silent stream the splice ends with nothing counted
    let grace = HALF_CLOSE_GRACE_SILENT + Duration::from_secs(1);
    let ended = tokio::time::timeout(grace, &mut p.task).await;
    let counts = ended.expect("the grace period must cut the stream");
    assert_eq!(counts.expect("the splice task didn't panic"), (0, 0));
}

/// A running splice whose site has already sent everything and closed.
struct Preloaded {
    /// The client writes here — this is `splice`'s `recv`.
    client_out: DuplexStream,
    site: TcpStream,
    task: JoinHandle<(u64, u64)>,
}

/// The site writes `site_bytes` (up to 48 KiB — what the socket buffers hold) and closes; after 50 ms
/// for delivery splice starts, so its tcp reader meets the end of the stream without ever waiting.
async fn preloaded(send: BoxSend, window: Option<usize>, site_bytes: &[u8]) -> Preloaded {
    let (tcp, mut site) = tcp_pair().await;
    site.write_all(site_bytes).await.expect("site write");
    site.shutdown().await.expect("site close");
    tokio::time::sleep(Duration::from_millis(50)).await;
    let (client_out, recv_side) = duplex(64 * 1024);
    let task = run_splice(send, Box::new(recv_side), tcp, window);
    Preloaded {
        client_out,
        site,
        task,
    }
}

/// A client stream that takes every byte and acknowledges only on request.
fn manual_send() -> (BoxSend, AckHandle) {
    let (writer, acks) = ack_writer(AckMode::Manual);
    (Box::new(writer), acks)
}

/// Acknowledges again and again until `total` bytes have reached the stream.
async fn release_all(acks: &AckHandle, total: usize) {
    within("every byte released", async {
        while acks.written_len() < total {
            acks.ack_all();
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
}

/// Both sides send and close at the same time; returns what the site and the client received.
async fn exchange(w: &mut Wires, up: &[u8], down: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let Wires {
        client_out,
        client_in,
        site,
        ..
    } = w;
    let (mut site_rd, mut site_wr) = site.split();
    let client_sends = async {
        client_out.write_all(up).await.expect("client write");
        client_out.shutdown().await.expect("client close");
    };
    let site_sends = async {
        site_wr.write_all(down).await.expect("site write");
        site_wr.shutdown().await.expect("site close");
    };
    let (_, _, at_site, at_client) = within("both directions", async {
        tokio::join!(
            client_sends,
            site_sends,
            read_all(&mut site_rd),
            read_all(client_in)
        )
    })
    .await;
    (at_site, at_client)
}
