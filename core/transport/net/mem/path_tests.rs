//! Tests of the `mem` path (`path.rs`): order, size, closing, queue pressure.

use super::*;
use crate::transport::net::mem::MemNet;
use crate::transport::net::{NodeSecret, PacketPath, PathKind, PeerId};
use bytes::Bytes;
use std::io::ErrorKind;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{sleep, timeout};

mod delivery {
    use super::*;

    #[tokio::test]
    async fn ten_thousand_packets_arrive_all_and_in_order() {
        let line = setup();

        let ((), got) = tokio::join!(
            async {
                for i in 0..10_000 {
                    line.a.send(pkt(i)).await.unwrap();
                }
            },
            async {
                let mut got = Vec::with_capacity(10_000);
                while got.len() < 10_000 {
                    got.push(num(&line.when_b_receives().await));
                }
                got
            }
        );

        assert_eq!(got, (0..10_000).collect::<Vec<u32>>());
    }

    #[tokio::test]
    async fn the_two_directions_do_not_mix() {
        let line = setup();

        line.a.send(pkt(1)).await.unwrap();
        line.b.send(pkt(2)).await.unwrap();

        assert_eq!(num(&line.when_b_receives().await), 1);
        assert_eq!(num(&line.when_a_receives().await), 2);
    }
}

mod packet_size {
    use super::*;

    #[test]
    fn the_limit_is_65536_bytes() {
        let line = setup();

        assert_eq!(line.a.max_packet_bytes(), 65_536);
        assert_eq!(line.b.max_packet_bytes(), 65_536);
        assert_eq!(MEM_MAX_PACKET, 65_536);
    }

    #[tokio::test]
    async fn a_packet_of_exactly_the_limit_goes_through() {
        let line = setup();

        line.a.send(Bytes::from(vec![7u8; 65_536])).await.unwrap();

        assert_eq!(line.when_b_receives().await.len(), 65_536);
    }

    #[tokio::test]
    async fn a_packet_one_byte_over_is_invalid_input() {
        let line = setup();

        let err = line
            .a
            .send(Bytes::from(vec![7u8; 65_537]))
            .await
            .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
    }

    #[tokio::test]
    async fn a_rejected_packet_is_not_delivered() {
        let line = setup();
        let _ = line.a.send(Bytes::from(vec![7u8; 65_537])).await;

        line.a.send(pkt(1)).await.unwrap();

        assert_eq!(
            num(&line.when_b_receives().await),
            1,
            "the next packet must arrive first"
        );
    }
}

mod closing_an_end {
    use super::*;

    #[tokio::test]
    async fn its_own_recv_reads_eof_at_once() {
        let line = setup();
        line.a.close();

        let err = line.recv_err_of_a().await;

        assert_eq!(err, ErrorKind::UnexpectedEof);
    }

    #[tokio::test]
    async fn its_own_recv_already_waiting_wakes_with_eof() {
        let line = setup();

        let (got, _) = tokio::join!(timeout(Duration::from_secs(2), line.a.recv()), async {
            sleep(Duration::from_millis(50)).await;
            line.a.close();
        });

        let err = got
            .expect("the waiting recv did not wake up on its own close")
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnexpectedEof);
    }

    #[tokio::test]
    async fn its_own_send_is_broken_pipe() {
        let line = setup();
        line.a.close();

        let err = line.a.send(pkt(1)).await.unwrap_err();

        assert_eq!(err.kind(), ErrorKind::BrokenPipe);
    }

    #[tokio::test]
    async fn the_far_end_reads_what_was_sent_before_the_close_then_eof() {
        let line = setup();
        for i in 0..3 {
            line.a.send(pkt(i)).await.unwrap();
        }
        line.a.close();

        let mut got = Vec::new();
        for _ in 0..3 {
            got.push(num(&line.when_b_receives().await));
        }

        assert_eq!(
            got,
            vec![0, 1, 2],
            "three packets before close must not be lost"
        );
        assert_eq!(line.recv_err_of_b().await, ErrorKind::UnexpectedEof);
    }

    #[tokio::test]
    async fn the_far_end_recv_already_waiting_wakes_with_eof() {
        let line = setup();

        let (got, _) = tokio::join!(timeout(Duration::from_secs(2), line.b.recv()), async {
            sleep(Duration::from_millis(50)).await;
            line.a.close();
        });

        let err = got
            .expect("the waiting recv did not wake up on the other side's close")
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnexpectedEof);
    }

    #[tokio::test]
    async fn the_far_end_send_is_broken_pipe() {
        let line = setup();
        line.a.close();

        let err = line.b.send(pkt(1)).await.unwrap_err();

        assert_eq!(err.kind(), ErrorKind::BrokenPipe);
    }

    #[tokio::test]
    async fn closing_twice_is_harmless() {
        let line = setup();

        line.a.close();
        line.a.close();

        assert_eq!(line.recv_err_of_a().await, ErrorKind::UnexpectedEof);
    }
}

mod dropping_an_end {
    use super::*;

    #[tokio::test]
    async fn the_far_end_reads_what_was_sent_then_eof() {
        let Line { a, b, .. } = setup();
        a.send(pkt(1)).await.unwrap();
        a.send(pkt(2)).await.unwrap();

        drop(a);

        assert_eq!(num(&recv_ok(&b).await), 1);
        assert_eq!(num(&recv_ok(&b).await), 2);
        assert_eq!(recv_err(&b).await, ErrorKind::UnexpectedEof);
    }

    #[tokio::test]
    async fn the_far_end_send_is_broken_pipe() {
        let Line { a, b, .. } = setup();

        drop(a);

        let err = b.send(pkt(1)).await.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::BrokenPipe);
    }
}

mod back_pressure {
    use super::*;

    #[tokio::test]
    async fn the_1025th_send_without_a_reader_waits_until_one_recv() {
        let line = setup();
        timeout(Duration::from_secs(2), async {
            for i in 0..1024 {
                line.a.send(pkt(i)).await.unwrap();
            }
        })
        .await
        .expect("the first 1024 packets must go out without waiting");

        let mut send_1025 = line.a.send(pkt(1024));
        let early = timeout(Duration::from_millis(100), &mut send_1025).await;
        assert!(
            early.is_err(),
            "the 1025th send finished without a reader: the queue does not hold pressure"
        );

        assert_eq!(num(&line.when_b_receives().await), 0);
        let late = timeout(Duration::from_secs(2), &mut send_1025).await;
        assert!(
            matches!(late, Ok(Ok(()))),
            "after one recv the send must finish"
        );
    }

    #[tokio::test]
    async fn a_waiting_send_loses_nothing() {
        let line = setup();
        let sender = async {
            for i in 0..1100 {
                line.a.send(pkt(i)).await.unwrap();
            }
        };
        let receiver = async {
            sleep(Duration::from_millis(100)).await;
            let mut got = Vec::new();
            for _ in 0..1100 {
                got.push(num(&line.when_b_receives().await));
            }
            got
        };

        let ((), got) = tokio::join!(sender, receiver);

        assert_eq!(got, (0..1100).collect::<Vec<u32>>());
    }
}

mod description {
    use super::*;

    #[test]
    fn each_end_names_the_other_one() {
        // The first path of the pair is end `a`, its `info().peer` is `b` (and vice versa).
        let line = setup();

        assert_eq!(line.a.info().peer, peer(2));
        assert_eq!(line.b.info().peer, peer(1));
    }

    #[test]
    fn is_direct_without_remote_in_the_mem_family() {
        let info = setup().a.info();

        assert!(
            matches!(info.kind, PathKind::Direct),
            "kind = {:?}",
            info.kind
        );
        assert_eq!(info.remote, None);
        assert_eq!(info.family, "mem");
    }

    #[test]
    fn as_any_is_the_path_itself() {
        let line = setup();

        let any = line
            .a
            .as_any()
            .downcast_ref::<MemPath>()
            .expect("as_any must return MemPath");

        assert!(
            std::ptr::eq(any, &line.a),
            "as_any returned not the path itself"
        );
    }
}

// ---- helpers ----

fn peer(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

fn pkt(i: u32) -> Bytes {
    Bytes::copy_from_slice(&i.to_be_bytes())
}

fn num(b: &Bytes) -> u32 {
    u32::from_be_bytes(b[..].try_into().expect("a packet of 4 bytes"))
}

async fn recv_ok(p: &MemPath) -> Bytes {
    let got = timeout(Duration::from_secs(5), p.recv()).await;
    got.expect("recv hung").expect("recv returned an error")
}

async fn recv_err(p: &MemPath) -> ErrorKind {
    let got = timeout(Duration::from_secs(2), p.recv()).await;
    got.expect("recv hung")
        .expect_err("recv must return an error")
        .kind()
}

/// A line of two ends: `a` is node 1, `b` is node 2.
struct Line {
    a: MemPath,
    b: MemPath,
    _net: Arc<MemNet>,
}

fn setup() -> Line {
    let net = MemNet::new();
    let (a, b) = MemPath::pair(&net.hub, (peer(1), 1), (peer(2), 2));
    Line { a, b, _net: net }
}

impl Line {
    async fn when_b_receives(&self) -> Bytes {
        recv_ok(&self.b).await
    }

    async fn when_a_receives(&self) -> Bytes {
        recv_ok(&self.a).await
    }

    async fn recv_err_of_a(&self) -> ErrorKind {
        recv_err(&self.a).await
    }

    async fn recv_err_of_b(&self) -> ErrorKind {
        recv_err(&self.b).await
    }
}
