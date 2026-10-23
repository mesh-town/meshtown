//! Tests of the `mem` connection (continuation of `conn_tests.rs`): packet size, parallel streams, connection name.

use super::*;

mod packet_size {
    use super::*;

    #[tokio::test]
    async fn a_single_write_takes_at_most_one_packet_of_payload() {
        let (a, _b) = conns();
        let mut out = open(&a).await;

        let n = out.send.write(&vec![1u8; 100_000]).await.expect("write");

        assert!(
            n > 0 && n <= MEM_MAX_PACKET - 5,
            "write accepted {n} bytes, the limit is {}",
            MEM_MAX_PACKET - 5
        );
    }

    #[tokio::test]
    async fn a_big_write_leaves_in_packets_that_fit_the_path() {
        let (conn, raw) = conn_with_raw();
        let mut out = open(&conn).await;
        let data = pattern(3 * MEM_MAX_PACKET, 5);
        within("write", out.send.write_all(&data))
            .await
            .expect("write");

        assert_eq!(recv_frame(&raw).await, Frame::Open(1));
        let got = collect_data(&raw, 1, data.len()).await;

        assert!(
            got == data,
            "assembled something else: {} bytes out of {}",
            got.len(),
            data.len()
        );
    }

    #[tokio::test]
    async fn a_write_of_three_packets_arrives_whole() {
        let (a, b) = conns();
        let mut out = open(&a).await;
        let mut inc = accept(&b).await;
        let data = pattern(3 * MEM_MAX_PACKET, 6);

        let ((), got) = both(
            write_all(&mut out.send, &data),
            read_exactly(&mut inc.recv, data.len()),
        )
        .await;

        assert!(
            got == data,
            "read something else: {} bytes out of {}",
            got.len(),
            data.len()
        );
    }
}

mod concurrency {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_hundred_parallel_echo_streams_come_back_intact() {
        let (a, b) = conns();
        crate::transport::net::task_label::spawn(echo_server(b));
        let mut set = JoinSet::new();
        for i in 0..100 {
            crate::transport::net::task_label::spawn_in(&mut set, echo_round_trip(a.clone(), i));
        }

        let bad = within_secs(30, "100 echo streams", failed_among(set)).await;

        assert_eq!(bad, Vec::<usize>::new(), "streams with a corrupted echo");
    }

    #[tokio::test]
    async fn an_echo_of_several_windows_on_one_stream_comes_back_intact() {
        let (a, b) = conns();
        crate::transport::net::task_label::spawn(echo_server(b));
        let data = pattern(3 * STREAM_WINDOW_BYTES, 4);

        let back = within_secs(20, "echo of three windows", echo_bytes(&a, &data)).await;

        assert!(
            back == data,
            "came back something else: {} bytes out of {}",
            back.len(),
            data.len()
        );
    }

    #[tokio::test]
    async fn streams_opened_by_both_sides_at_once_do_not_clash() {
        let (a, b) = conns();

        let (received_by_b, received_by_a) = within_secs(
            20,
            "streams from both sides",
            both(
                tags_received_after_sending(&a, &b, "a"),
                tags_received_after_sending(&b, &a, "b"),
            ),
        )
        .await;

        assert_eq!(received_by_b, tags("a", 5), "what A opened");
        assert_eq!(received_by_a, tags("b", 5), "what B opened");
    }
}

mod identity {
    use super::*;

    #[tokio::test]
    async fn each_conn_names_the_far_node_as_its_peer() {
        let (a, b) = conns();

        assert_eq!(a.peer(), peer(2));
        assert_eq!(b.peer(), peer(1));
    }

    #[tokio::test]
    async fn two_conns_have_different_stable_ids() {
        let (a, b) = conns();

        assert_ne!(a.stable_id(), b.stable_id());
    }

    #[tokio::test]
    async fn the_stable_id_of_a_conn_does_not_change() {
        let (a, _b) = conns();

        assert_eq!(a.stable_id(), a.stable_id());
    }
}

// ---- helpers ----

/// A dialing connection; the other end of the line is a "raw" path without `MemConn`.
fn conn_with_raw() -> (Arc<MemConn>, MemPath) {
    let (pa, raw) = paths();
    (MemConn::spawn(Arc::new(pa), true), raw)
}

fn tags(side: &str, n: usize) -> Vec<String> {
    (0..n).map(|i| format!("{side} {i}")).collect()
}

async fn recv_packet(raw: &MemPath) -> Bytes {
    let got = within("recv of the raw path", raw.recv()).await;
    got.expect("recv of the raw path returned an error")
}

async fn recv_frame(raw: &MemPath) -> Frame {
    Frame::decode(recv_packet(raw).await).expect("what arrived is not a frame")
}

/// Reads the `Data` frames of stream `id` from the raw path until `len` bytes are collected; each frame is ≤ one packet.
async fn collect_data(raw: &MemPath, id: u32, len: usize) -> Vec<u8> {
    let mut got = Vec::new();
    while got.len() < len {
        let pkt = recv_packet(raw).await;
        assert!(
            pkt.len() <= MEM_MAX_PACKET,
            "packet of {} bytes is longer than the limit",
            pkt.len()
        );
        match Frame::decode(pkt).expect("what arrived is not a frame") {
            Frame::Data(got_id, part) if got_id == id => got.extend_from_slice(&part),
            other => panic!("expected Data({id}, ..), got {other:?}"),
        }
    }
    got
}

/// Accepts streams and sends everything read back, while the connection is alive.
async fn echo_server(conn: Arc<MemConn>) {
    let mut tasks = JoinSet::new();
    while let Ok(StreamPair { mut send, mut recv }) = conn.accept().await {
        crate::transport::net::task_label::spawn_in(&mut tasks, async move {
            tokio::io::copy(&mut recv, &mut send)
                .await
                .expect("echo: copy");
            send.shutdown().await.expect("echo: shutdown");
        });
    }
}

async fn echo_bytes(conn: &Arc<MemConn>, data: &[u8]) -> Vec<u8> {
    let StreamPair { mut send, mut recv } = open(conn).await;
    let (_, back) = tokio::join!(write_all(&mut send, data), read_to_end(&mut recv));
    back
}

/// The stream number, if the echo came back different from what went out.
async fn echo_round_trip(conn: Arc<MemConn>, i: usize) -> Option<usize> {
    let data = pattern(40_000, i);
    let back = echo_bytes(&conn, &data).await;
    (back != data).then_some(i)
}

async fn failed_among(mut set: JoinSet<Option<usize>>) -> Vec<usize> {
    let mut bad = Vec::new();
    while let Some(res) = set.join_next().await {
        bad.extend(res.expect("echo task panicked"));
    }
    bad
}

/// `from` opens 5 streams with tags and closes the write side; `to` accepts them and reads the tags.
async fn tags_received_after_sending(
    from: &Arc<MemConn>,
    to: &Arc<MemConn>,
    label: &str,
) -> Vec<String> {
    let mut opened = Vec::new();
    for tag in tags(label, 5) {
        let mut pair = open(from).await;
        write_all(&mut pair.send, tag.as_bytes()).await;
        opened.push(pair);
    }
    let mut got = Vec::new();
    for _ in 0..5 {
        let mut inc = accept(to).await;
        got.push(String::from_utf8(read_to_end(&mut inc.recv).await).expect("the tag is utf-8"));
    }
    got.sort();
    got
}
