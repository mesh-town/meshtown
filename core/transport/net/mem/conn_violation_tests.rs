//! Tests of the `mem` connection (continuation of `conn_close_tests.rs`): protocol violations, stream ids, byte accounting.

use super::*;

mod protocol_violation {
    use super::*;

    #[tokio::test]
    async fn raw_data_beyond_the_window_is_a_violation() {
        let (conn, raw) = conn_with_raw(true);
        send_frame(&raw, Frame::Open(2)).await;
        send_window_of_data(&raw, 2).await;

        send_frame(&raw, Frame::Data(2, Bytes::from_static(b"x"))).await;

        assert_eq!(close_code_seen_by(&raw).await, CODE_PROTOCOL);
        assert_eq!(conn.close_info().map(|(code, _)| code), Some(CODE_PROTOCOL));
    }

    #[tokio::test]
    async fn exactly_one_window_of_raw_data_is_fine() {
        let (conn, raw) = conn_with_raw(true);
        send_frame(&raw, Frame::Open(2)).await;
        send_window_of_data(&raw, 2).await;
        let mut inc = accept(&conn).await;

        read_exactly(&mut inc.recv, STREAM_WINDOW_BYTES).await;

        assert_eq!(conn.close_info(), None);
    }

    #[tokio::test]
    async fn data_on_a_stream_never_opened_is_a_violation() {
        let (_conn, raw) = conn_with_raw(true);

        send_frame(&raw, Frame::Data(2, Bytes::from_static(b"x"))).await;

        assert_eq!(close_code_seen_by(&raw).await, CODE_PROTOCOL);
    }

    #[tokio::test]
    async fn data_on_a_removed_stream_is_dropped_silently() {
        let (conn, raw) = conn_with_raw(true);
        send_frame(&raw, Frame::Open(2)).await;
        send_frame(&raw, Frame::Fin(2)).await;
        let mut stream = accept(&conn).await;
        stream.send.shutdown().await.expect("shutdown");
        assert_eq!(
            recv_frame(&raw).await,
            Frame::Fin(2),
            "Fin of the connection"
        );

        send_frame(&raw, Frame::Data(2, Bytes::from_static(b"late"))).await;
        send_frame(&raw, Frame::Open(4)).await;

        let _probe = accept(&conn).await;
        assert_eq!(conn.close_info(), None);
    }

    #[tokio::test]
    async fn exactly_1024_incoming_streams_are_allowed() {
        let (conn, raw) = conn_with_raw(true);
        open_incoming_streams(&raw, 1024).await;

        let mut held = Vec::new();
        for _ in 0..1024 {
            held.push(accept(&conn).await);
        }

        assert_eq!(conn.close_info(), None);
    }

    #[tokio::test]
    async fn the_1025th_incoming_stream_is_a_violation() {
        let (_conn, raw) = conn_with_raw(true);

        open_incoming_streams(&raw, 1025).await;

        assert_eq!(close_code_seen_by(&raw).await, CODE_PROTOCOL);
    }
}

mod stream_ids {
    use super::*;

    #[tokio::test]
    async fn the_dialer_opens_odd_ids_from_one() {
        let (conn, raw) = conn_with_raw(true);
        let _streams = [open(&conn).await, open(&conn).await, open(&conn).await];

        let opens = [
            recv_frame(&raw).await,
            recv_frame(&raw).await,
            recv_frame(&raw).await,
        ];

        assert_eq!(opens, [Frame::Open(1), Frame::Open(3), Frame::Open(5)]);
    }

    #[tokio::test]
    async fn the_acceptor_opens_even_ids_from_two() {
        let (conn, raw) = conn_with_raw(false);
        let _streams = [open(&conn).await, open(&conn).await];

        let opens = [recv_frame(&raw).await, recv_frame(&raw).await];

        assert_eq!(opens, [Frame::Open(2), Frame::Open(4)]);
    }
}

mod link_counters {
    use super::*;

    #[tokio::test]
    async fn sent_bytes_grow_when_data_is_written() {
        let (a, _b) = conns();
        let before = a.link().sent_bytes;
        let mut out = open(&a).await;

        out.send.write_all(&[1u8; 10_000]).await.expect("write");

        eventually("sent_bytes grew by the size of the write", || {
            a.link().sent_bytes >= before + 10_000
        })
        .await;
    }

    #[tokio::test]
    async fn the_far_side_recv_bytes_grow_with_the_data() {
        let (a, b) = conns();
        let before = b.link().recv_bytes;
        let mut out = open(&a).await;

        out.send.write_all(&[1u8; 10_000]).await.expect("write");

        eventually("recv_bytes grew by the size of the write", || {
            b.link().recv_bytes >= before + 10_000
        })
        .await;
    }

    #[tokio::test]
    async fn link_describes_a_direct_mem_path_without_rtt() {
        let (a, _b) = conns();

        let link = a.link();

        assert!(
            matches!(link.path, PathKind::Direct),
            "path: {:?}",
            link.path
        );
        assert_eq!(link.remote, None);
        assert_eq!(link.rtt, None);
        assert_eq!(link.lost_packets, 0);
        assert_eq!(link.cwnd, None);
    }
}

// ---- helpers ----

async fn eventually(what: &str, mut cond: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(5);
    while !cond() {
        assert!(Instant::now() < until, "{what}: did not happen within 5 s");
        sleep(Duration::from_millis(10)).await;
    }
}

async fn send_frame(raw: &MemPath, frame: Frame) {
    raw.send(frame.encode())
        .await
        .expect("send of a frame to the raw path");
}

async fn recv_frame(raw: &MemPath) -> Frame {
    let pkt = within("recv of the raw path", raw.recv()).await;
    Frame::decode(pkt.expect("recv of the raw path returned an error"))
        .expect("what arrived is not a frame")
}

/// Code of the first `Close` that arrived on the raw path (other frames are skipped).
async fn close_code_seen_by(raw: &MemPath) -> u32 {
    for _ in 0..16 {
        if let Frame::Close { code, .. } = recv_frame(raw).await {
            return code;
        }
    }
    panic!("Close did not arrive within 16 frames");
}

/// Exactly one window of data in 32 KiB frames.
async fn send_window_of_data(raw: &MemPath, id: u32) {
    let part = Bytes::from(vec![0u8; 32 * 1024]);
    for _ in 0..STREAM_WINDOW_BYTES / part.len() {
        send_frame(raw, Frame::Data(id, part.clone())).await;
    }
}

/// `n` incoming streams with even ids 2, 4, …; the connection is the dialing one.
async fn open_incoming_streams(raw: &MemPath, n: u32) {
    for i in 1..=n {
        send_frame(raw, Frame::Open(2 * i)).await;
    }
}
