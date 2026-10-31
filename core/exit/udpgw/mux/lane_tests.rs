use super::*;
use crate::exit::udpgw::codec::{encode, parse_body, Frame, Target, FLAG_DATA};
use crate::exit::udpgw::test_support::{cfg_with, spawn_echo, start_streams};
use std::collections::HashMap;
use std::time::Duration;
use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt, DuplexStream};
use tokio::time::timeout;

const REQ_DATA_DOMAIN: &str = "0014020002030b6578616d706c652e636f6d007b0102";
const REQ_DATA_V6: &str = "00180200030420010db800000000000000000000000100357636";
const REQ_KEEPALIVE_7: &str = "0003010007";
const REQ_UNKNOWN_FLAG_9: &str = "0003080009";
const REQ_ERR_4: &str = "0003200004";
const RESP_ERR_9: &str = "0003200009";
const WAIT: Duration = Duration::from_secs(2);
const SHORT: Duration = Duration::from_millis(250);

fn bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex"))
        .collect()
}

fn v4_data(conn_id: u16, payload: &[u8]) -> Vec<u8> {
    let from = "192.0.2.10:53".parse().expect("addr");
    encode(&Frame::data(conn_id, from, payload.to_vec())).expect("fits")
}

fn flagged(flags: u8, conn_id: u16) -> Vec<u8> {
    let frame = Frame {
        flags,
        conn_id,
        target: None,
        data: Vec::new(),
    };
    encode(&frame).expect("fits")
}

fn to_echo(conn_id: u16, payload: &[u8]) -> Vec<u8> {
    let target = Some(Target::Domain("echo.test".into(), 9));
    let frame = Frame {
        flags: FLAG_DATA,
        conn_id,
        target,
        data: payload.to_vec(),
    };
    encode(&frame).expect("fits")
}

/// A local connection as the user of the lane sees it.
struct Conn {
    send: BoxSend,
    recv: BoxRecv,
}

impl Conn {
    fn of((send, recv): (BoxSend, BoxRecv)) -> Conn {
        Conn { send, recv }
    }

    async fn put(&mut self, bytes: &[u8]) {
        self.send.write_all(bytes).await.expect("sent");
    }

    async fn raw(&mut self) -> RawFrame {
        timeout(WAIT, read_raw(&mut self.recv))
            .await
            .expect("in time")
            .expect("frame")
    }

    async fn frame(&mut self) -> Frame {
        parse_body(self.raw().await.body()).expect("parsed")
    }

    async fn sees_the_end(&mut self) {
        let end = timeout(Duration::from_secs(1), self.recv.read_u8()).await;
        assert!(end.expect("ends in time").is_err());
    }

    async fn silent(&mut self) -> bool {
        timeout(SHORT, self.recv.read_u8()).await.is_err()
    }
}

/// The exit side of a lane over plain pipes: the test reads and writes the stream itself.
struct Far {
    from_lane: DuplexStream,
    to_lane: DuplexStream,
}

impl Far {
    async fn next(&mut self) -> RawFrame {
        timeout(WAIT, read_raw(&mut self.from_lane))
            .await
            .expect("in time")
            .expect("frame")
    }

    async fn put(&mut self, bytes: &[u8]) {
        self.to_lane.write_all(bytes).await.expect("sent");
    }

    async fn silent(&mut self) -> bool {
        timeout(SHORT, self.from_lane.read_u8()).await.is_err()
    }
}

fn raw_lane() -> (Arc<Lane>, Conn, Far) {
    let (lane_send, from_lane) = duplex(1 << 16);
    let (lane_recv, to_lane) = duplex(1 << 16);
    let (lane, first) = Lane::start(Box::new(lane_send), Box::new(lane_recv));
    (lane, Conn::of(first), Far { from_lane, to_lane })
}

async fn server_lane() -> (Arc<Lane>, Conn) {
    let echo = spawn_echo().await;
    let cfg = cfg_with(HashMap::from([("echo.test".to_string(), echo)]));
    let (send, recv, _task) = start_streams(cfg);
    let (lane, first) = Lane::start(send, recv);
    (lane, Conn::of(first))
}

fn echo_of(conn_id: u16, payload: &[u8], echo: std::net::SocketAddr) -> Frame {
    Frame::data(conn_id, echo, payload.to_vec())
}

mod through_the_exit {
    use super::*;

    #[tokio::test]
    async fn two_locals_with_the_same_conn_id_get_their_own_echo() {
        let echo = spawn_echo().await;
        let cfg = cfg_with(HashMap::from([("echo.test".to_string(), echo)]));
        let (send, recv, _task) = start_streams(cfg);
        let (lane, first) = Lane::start(send, recv);
        let mut a = Conn::of(first);
        let mut b = Conn::of(lane.attach().expect("attached"));
        a.put(&to_echo(1, b"from a")).await;
        b.put(&to_echo(1, b"from b")).await;
        assert_eq!(a.frame().await, echo_of(1, b"from a", echo));
        assert_eq!(b.frame().await, echo_of(1, b"from b", echo));
    }

    #[tokio::test]
    async fn malformed_data_ends_only_its_local() {
        let (lane, first) = server_lane().await;
        let mut a = first;
        let mut b = Conn::of(lane.attach().expect("attached"));
        a.put(&[0, 6, 0x02, 0, 1, 9, 0, 0]).await;
        a.sees_the_end().await;
        b.put(&to_echo(1, b"still here")).await;
        assert_eq!(b.frame().await.data, b"still here");
        assert!(!lane.is_dead());
    }
}

mod forwarding {
    use super::*;

    #[tokio::test]
    async fn domain_and_ipv6_frames_pass_byte_for_byte() {
        let (_lane, mut conn, mut far) = raw_lane();
        for (index, hex) in [REQ_DATA_DOMAIN, REQ_DATA_V6].into_iter().enumerate() {
            let sent = bytes(hex);
            conn.put(&sent).await;
            let got = far.next().await.into_bytes();
            assert_eq!(got[3..5], [0, index as u8 + 1]);
            assert_eq!((&got[..3], &got[5..]), (&sent[..3], &sent[5..]));
        }
    }

    #[tokio::test]
    async fn close_goes_out_with_the_shared_id_and_frees_it() {
        let (_lane, mut conn, mut far) = raw_lane();
        conn.put(&v4_data(5, b"x")).await;
        let shared = far.next().await.conn_id();
        conn.put(&flagged(0x04, 5)).await;
        let closed = far.next().await;
        assert_eq!((closed.kind(), closed.conn_id()), (Kind::Close, shared));
        far.put(&v4_data(shared, b"late")).await;
        assert!(conn.silent().await);
    }

    #[tokio::test]
    async fn close_of_an_unknown_id_is_silent() {
        let (_lane, mut conn, mut far) = raw_lane();
        conn.put(&flagged(0x04, 9)).await;
        assert!(far.silent().await);
    }

    #[tokio::test]
    async fn full_lane_drops_data_of_a_new_binding() {
        let (_lane, mut conn, mut far) = raw_lane();
        for id in 0..257u16 {
            conn.put(&v4_data(id, b"x")).await;
        }
        for _ in 0..256 {
            far.next().await;
        }
        assert!(far.silent().await);
    }
}

mod local_answers {
    use super::*;

    #[tokio::test]
    async fn keepalive_is_answered_locally() {
        let (_lane, mut conn, mut far) = raw_lane();
        conn.put(&bytes(REQ_KEEPALIVE_7)).await;
        assert_eq!(conn.raw().await.into_bytes(), bytes(REQ_KEEPALIVE_7));
        assert!(far.silent().await);
    }

    #[tokio::test]
    async fn unknown_flag_and_error_get_the_reference_error_locally() {
        let (_lane, mut conn, mut far) = raw_lane();
        conn.put(&bytes(REQ_UNKNOWN_FLAG_9)).await;
        assert_eq!(conn.raw().await.into_bytes(), bytes(RESP_ERR_9));
        conn.put(&bytes(REQ_ERR_4)).await;
        assert_eq!(conn.raw().await.into_bytes(), bytes(REQ_ERR_4));
        assert!(far.silent().await);
    }
}

mod leaks {
    use super::*;

    #[tokio::test]
    async fn local_gone_without_close_sends_close_for_each_id() {
        let (lane, mut conn, mut far) = raw_lane();
        let mut shared = Vec::new();
        for id in 1..=3 {
            conn.put(&v4_data(id, b"x")).await;
            shared.push(far.next().await.conn_id());
        }
        drop(conn);
        let mut closed = Vec::new();
        for _ in 0..3 {
            let frame = far.next().await;
            assert_eq!(frame.kind(), Kind::Close);
            closed.push(frame.conn_id());
        }
        shared.sort_unstable();
        closed.sort_unstable();
        assert_eq!(closed, shared);
        assert_eq!(lock(&lane.table).len(), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn idle_binding_is_closed_on_next_bind() {
        let (_lane, mut conn, mut far) = raw_lane();
        conn.put(&v4_data(1, b"x")).await;
        let old = far.next().await.conn_id();
        tokio::time::advance(SESSION_IDLE + Duration::from_secs(1)).await;
        conn.put(&v4_data(2, b"y")).await;
        let closed = far.next().await;
        assert_eq!((closed.kind(), closed.conn_id()), (Kind::Close, old));
        assert_eq!(far.next().await.kind(), Kind::Data);
    }
}

mod death {
    use super::*;

    #[tokio::test]
    async fn lane_eof_ends_every_local() {
        let (lane, first, far) = raw_lane();
        let mut locals = vec![first];
        locals.extend((0..2).map(|_| Conn::of(lane.attach().expect("attached"))));
        assert_eq!(lane.locals(), 3);
        drop(far);
        for local in &mut locals {
            local.sees_the_end().await;
        }
        assert!(lane.attach().is_none());
    }

    #[tokio::test]
    async fn lane_write_error_ends_every_local() {
        let (lane, mut first, far) = raw_lane();
        let mut second = Conn::of(lane.attach().expect("attached"));
        drop(far.from_lane);
        first.put(&v4_data(1, b"x")).await;
        first.sees_the_end().await;
        second.sees_the_end().await;
        assert!(timeout(WAIT, lane.ended()).await.is_ok());
    }

    #[tokio::test]
    async fn broken_frame_from_the_exit_ends_the_lane() {
        let (lane, mut conn, mut far) = raw_lane();
        far.put(&[0, 2, 1, 0]).await;
        conn.sees_the_end().await;
        assert!(lane.attach().is_none());
    }

    #[tokio::test]
    async fn close_finishes_the_stream_and_ends_the_lane() {
        let (lane, mut conn, mut far) = raw_lane();
        lane.close();
        assert!(timeout(WAIT, lane.ended()).await.is_ok());
        assert!(far.from_lane.read_u8().await.is_err());
        conn.sees_the_end().await;
    }
}

mod replies {
    use super::*;

    #[tokio::test]
    async fn exit_error_frame_reaches_its_local_with_the_local_id() {
        let (_lane, mut conn, mut far) = raw_lane();
        conn.put(&v4_data(5, b"x")).await;
        let shared = far.next().await.conn_id();
        far.put(&flagged(0x20, shared)).await;
        assert_eq!(conn.raw().await.into_bytes(), flagged(0x20, 5));
    }

    #[tokio::test]
    async fn exit_keepalive_is_swallowed() {
        let (_lane, mut conn, mut far) = raw_lane();
        conn.put(&v4_data(5, b"x")).await;
        let shared = far.next().await.conn_id();
        far.put(&flagged(0x01, shared)).await;
        assert!(conn.silent().await);
    }

    #[tokio::test]
    async fn reply_for_an_unknown_shared_id_is_dropped_and_others_flow() {
        let (_lane, mut conn, mut far) = raw_lane();
        conn.put(&v4_data(5, b"x")).await;
        let shared = far.next().await.conn_id();
        far.put(&flagged(0x20, 999)).await;
        far.put(&flagged(0x20, shared)).await;
        assert_eq!(conn.raw().await.into_bytes(), flagged(0x20, 5));
        assert!(conn.silent().await);
    }

    #[tokio::test]
    async fn slow_local_does_not_hold_replies_of_another() {
        let (lane, mut a, mut far) = raw_lane();
        let mut b = Conn::of(lane.attach().expect("attached"));
        a.put(&v4_data(1, b"x")).await;
        b.put(&v4_data(1, b"y")).await;
        let (shared_a, shared_b) = (far.next().await.conn_id(), far.next().await.conn_id());
        for _ in 0..(LOCAL_QUEUE * 3) {
            far.put(&flagged(0x20, shared_a)).await;
        }
        far.put(&flagged(0x20, shared_b)).await;
        assert_eq!(b.raw().await.into_bytes(), flagged(0x20, 1));
    }
}
