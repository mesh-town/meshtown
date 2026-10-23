//! Tests for `splice` (`splice.rs`): the other side's grace period, closing the halves, byte counting, dropping the future.
//! Stream boxes are `tokio::io::duplex`, `tcp` is a paired `TcpListener` on `127.0.0.1:0`.

use super::*;
use crate::transport::net::facade::test_support::within;
use tokio::io::{duplex, AsyncRead, AsyncReadExt, AsyncWriteExt, DuplexStream};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

mod wait_for_second_side {
    use super::*;

    #[test]
    fn grace_after_zero_bytes_is_silent() {
        // given: the first side to close sent no bytes (a browser pre-connection)
        let first_bytes = 0;

        // when: we compute the other side's wait time
        let wait = half_close_grace_after(first_bytes);

        // then: a short one — `HALF_CLOSE_GRACE_SILENT`, 5 s
        assert_eq!(wait, HALF_CLOSE_GRACE_SILENT);
        assert_eq!(
            wait,
            Duration::from_secs(5),
            "the deadline for a silent stream"
        );
    }

    #[test]
    fn grace_after_traffic_is_full() {
        // given: the first side to close sent at least one byte
        let first_bytes = 1;

        // when: we compute the other side's wait time
        let wait = half_close_grace_after(first_bytes);

        // then: the full one — `HALF_CLOSE_GRACE`, 60 s
        assert_eq!(wait, HALF_CLOSE_GRACE);
        assert_eq!(
            wait,
            Duration::from_secs(60),
            "the deadline for a stream with traffic doesn't change"
        );
    }
}

mod recv_end {
    use super::*;

    #[tokio::test]
    async fn shuts_tcp_write_so_site_reader_sees_eof() {
        // given: the client sent a word and closed its side of the stream
        let mut w = setup().await;
        w.client_out
            .write_all(b"hello")
            .await
            .expect("client write");
        w.client_out.shutdown().await.expect("client close");

        // when: the site reads to the end
        let got = within("site read to EOF", read_all(&mut w.site)).await;

        // then: it got all the bytes and saw the end
        assert_eq!(got, b"hello", "the client's bytes reached the site in full");
    }

    #[tokio::test]
    async fn empty_stream_on_both_ends_gives_zero_zero() {
        // given: the client closed the stream without sending a byte
        let mut w = setup().await;
        w.client_out.shutdown().await.expect("client close");

        // when: the site also closes its connection
        w.site.shutdown().await.expect("site close");

        // then: splice returned (0, 0) without waiting for the grace period
        assert_eq!(finished(&mut w).await, (0, 0));
    }
}

mod tcp_end {
    use super::*;

    #[tokio::test]
    async fn shuts_send_so_client_reader_sees_eof_after_all_bytes() {
        // given: the site sent four bytes and closed the connection
        let mut w = setup().await;
        w.site.write_all(b"data").await.expect("site write");
        w.site.shutdown().await.expect("site close");

        // when: the client reads its half of the stream to the end
        let got = within("client read to EOF", read_all(&mut w.client_in)).await;

        // then: all the bytes arrived and the end of the stream follows
        assert_eq!(got, b"data", "the site's bytes reached the client in full");
    }
}

mod byte_counts {
    use super::*;

    #[tokio::test]
    async fn pair_is_client_to_site_then_site_to_client() {
        // given: the client gave the site 10,000 bytes and closed, the site answered with three and closed
        let mut w = setup().await;
        w.client_out
            .write_all(&vec![7u8; 10_000])
            .await
            .expect("client write");
        w.client_out.shutdown().await.expect("client close");
        let mut got = vec![0u8; 10_000];
        within("site received everything", w.site.read_exact(&mut got))
            .await
            .expect("site read");
        w.site.write_all(b"abc").await.expect("site write");
        w.site.shutdown().await.expect("site close");

        // when: splice finished both directions
        let counts = finished(&mut w).await;

        // then: (client→site, site→client)
        assert_eq!(counts, (10_000, 3));
    }
}

mod dropped_future {
    use super::*;

    #[tokio::test]
    async fn stops_both_halves() {
        // given: splice is running, both subtasks already started
        let mut w = setup().await;
        tokio::time::sleep(Duration::from_millis(50)).await;

        // when: the splice future is dropped before the streams end
        w.task.abort();
        let _ = (&mut w.task).await;

        // then: the site sees the end, the client gets no data and can't write
        let mut buf = [0u8; 1];
        let site = within("site saw the end", w.site.read(&mut buf)).await;
        assert_eq!(site.expect("site read"), 0, "the tcp half is closed");
        let client = within("client saw the end", w.client_in.read(&mut buf)).await;
        assert_eq!(client.expect("client read"), 0, "the send half is closed");
        let write = within("client write", w.client_out.write_all(b"x")).await;
        assert!(write.is_err(), "the recv half is dropped: nowhere to write");
    }
}

/// Wire ends of `splice`: what the client and the site see, and the task itself.
struct Wires {
    /// The client writes here — this is `splice`'s `recv`.
    client_out: DuplexStream,
    /// The client reads from here — this is `splice`'s `send`.
    client_in: DuplexStream,
    /// The site: the other end of the tcp connection handed to `splice`.
    site: TcpStream,
    task: JoinHandle<(u64, u64)>,
}

/// A running `splice` on a `duplex` pair and a tcp pair.
async fn setup() -> Wires {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
    let addr = listener.local_addr().expect("listener address");
    let (tcp, accepted) = tokio::join!(TcpStream::connect(addr), listener.accept());
    let tcp = tcp.expect("connection to the listener");
    let (site, _) = accepted.expect("accepting the connection");
    let (client_out, recv_side) = duplex(64 * 1024);
    let (send_side, client_in) = duplex(64 * 1024);
    let (send, recv): (BoxSend, BoxRecv) = (Box::new(send_side), Box::new(recv_side));
    let task = crate::node::output::spawn(splice(send, recv, tcp));
    Wires {
        client_out,
        client_in,
        site,
        task,
    }
}

/// The result of `splice` (up to `STEP`).
async fn finished(w: &mut Wires) -> (u64, u64) {
    within("splice finished", &mut w.task)
        .await
        .expect("the splice task didn't panic")
}

/// Everything left in the reader, to the end of the stream.
async fn read_all<R: AsyncRead + Unpin>(reader: &mut R) -> Vec<u8> {
    let mut out = Vec::new();
    reader.read_to_end(&mut out).await.expect("read to end");
    out
}
