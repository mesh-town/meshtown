//! Tests of the `mem` carrier (`carrier.rs`): the handshake on a `MemPath` pair directly, refusals, the time limit.

use crate::net::mem::carrier::MemCarrier;
use crate::net::mem::frame::Frame;
use crate::net::mem::{MemNet, MemPath};
use crate::net::{
    Carrier, NetError, NetResult, NodeSecret, PacketPath, PathInfo, PathKind, PeerConn, PeerId,
    CarrierRole, ServiceId, StreamPair, AUTH, EXIT, PORT,
};
use bytes::Bytes;
use std::io::ErrorKind;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::timeout;

type Started = NetResult<(ServiceId, Arc<dyn PeerConn>)>;

mod accepting {
    use super::*;

    #[tokio::test]
    async fn the_returned_name_is_the_listed_static_not_the_dialers_copy() {
        let listed = ServiceId::new(b"test/listed/1");
        let dialed = copy_of(listed);

        let (_, accepted) = handshake(dialed, &[AUTH, listed, EXIT]).await;

        let (svc, _conn) = accepted.expect("a name from the list must be accepted");
        assert_eq!(
            svc.as_bytes().as_ptr(),
            listed.as_bytes().as_ptr(),
            "the acceptor returned something other than a &'static from its list"
        );
    }

    #[tokio::test]
    async fn the_dialer_gets_back_the_name_it_asked_for() {
        let (dialed, _) = handshake(EXIT, &[AUTH, EXIT]).await;

        let (svc, _conn) = dialed.expect("the dialer must be accepted");

        assert_eq!(svc, EXIT);
    }

    #[tokio::test]
    async fn the_right_name_is_picked_out_of_several() {
        let (_, accepted) = handshake(PORT, &[AUTH, EXIT, PORT]).await;

        let (svc, _conn) = accepted.expect("a name from the list must be accepted");

        assert_eq!(svc, PORT);
    }

    #[tokio::test]
    async fn the_dialer_speaks_first_with_a_hello_holding_the_service_bytes() {
        let (a, b) = setup();
        let carrier = MemCarrier::with_timeout(Duration::from_millis(300));

        let (_, first) = tokio::join!(carrier.start(Box::new(a), CarrierRole::Dial(EXIT)), recv_frame(&b));

        assert_eq!(first, Frame::Hello(Bytes::from_static(EXIT.as_bytes())));
    }

    #[tokio::test]
    async fn bytes_sent_right_after_accept_reach_the_acceptors_stream() {
        let (a, b) = setup();
        let carrier = MemCarrier::new();

        let (started, ()) = tokio::join!(
            carrier.start(Box::new(a), CarrierRole::Accept(list_of(&[EXIT]))),
            when_a_raw_dialer_opens_a_stream_at_once(&b)
        );

        let (_, conn) = started.expect("the acceptor must accept");
        let stream = accept_stream(&conn).await;
        assert_eq!(read_all(stream).await, b"early");
    }
}

mod refusing {
    use super::*;

    #[tokio::test]
    async fn a_foreign_name_is_refused_on_both_sides() {
        let (dialed, accepted) = handshake(EXIT, &[AUTH]).await;

        assert_refused(dialed, "dialer");
        assert_refused(accepted, "acceptor");
    }

    #[tokio::test]
    async fn an_empty_list_refuses_every_name() {
        let (dialed, accepted) = handshake(AUTH, &[]).await;

        assert_refused(dialed, "dialer");
        assert_refused(accepted, "acceptor");
    }

    #[tokio::test]
    async fn the_acceptor_answers_refuse_and_closes_the_path() {
        let (a, b) = setup();
        let carrier = MemCarrier::new();

        let (started, ()) = tokio::join!(
            carrier.start(Box::new(a), CarrierRole::Accept(list_of(&[AUTH]))),
            async {
                send_frame(&b, hello(EXIT)).await;
                assert_eq!(recv_frame(&b).await, Frame::Refuse);
                assert_eq!(
                    recv_err(&b).await,
                    ErrorKind::UnexpectedEof,
                    "after a refusal the path must be closed"
                );
            }
        );

        assert_refused(started, "acceptor");
    }

    #[tokio::test]
    async fn a_dialer_refused_by_the_far_end_closes_its_path() {
        let (a, b) = setup();
        let carrier = MemCarrier::new();

        let (started, ()) = tokio::join!(carrier.start(Box::new(a), CarrierRole::Dial(EXIT)), async {
            recv_frame(&b).await;
            send_frame(&b, Frame::Refuse).await;
        });

        assert_refused(started, "dialer");
        assert_eq!(recv_err(&b).await, ErrorKind::UnexpectedEof);
    }

    #[tokio::test]
    async fn a_dialer_whose_far_end_hangs_up_is_refused() {
        let (a, b) = setup();
        let carrier = MemCarrier::new();

        let (started, ()) = tokio::join!(carrier.start(Box::new(a), CarrierRole::Dial(EXIT)), async {
            recv_frame(&b).await;
            b.close();
        });

        assert_refused(started, "dialer");
    }
}

#[path = "carrier_fail_tests.rs"]
mod broken_handshake;

mod carrying {
    use super::*;

    #[test]
    fn kind_is_mem() {
        assert_eq!(MemCarrier::new().kind(), "mem");
    }

    #[test]
    fn a_path_of_the_mem_family_can_be_carried() {
        assert!(MemCarrier::new().can_carry(&info_of_family("mem")));
    }

    #[test]
    fn paths_of_other_families_cannot_be_carried() {
        let carrier = MemCarrier::new();

        for family in ["udx", "udp", "iroh", ""] {
            assert!(
                !carrier.can_carry(&info_of_family(family)),
                "a path of family {family:?} is considered usable"
            );
        }
    }

    #[test]
    fn the_info_of_a_real_mem_path_can_be_carried() {
        let (a, _b) = setup();

        assert!(MemCarrier::new().can_carry(&a.info()));
    }
}

mod after_the_handshake {
    use super::*;

    #[tokio::test]
    async fn each_conn_names_the_far_node_as_its_peer() {
        let (dialer, acceptor) = connected().await;

        assert_eq!(dialer.peer(), peer(2));
        assert_eq!(acceptor.peer(), peer(1));
    }

    #[tokio::test]
    async fn a_stream_opened_by_the_dialer_carries_bytes_to_the_acceptor() {
        let (dialer, acceptor) = connected().await;
        let mut out = open_stream(&dialer).await;
        out.send.write_all(b"ping").await.expect("write to the stream");

        let mut inc = accept_stream(&acceptor).await;

        assert_eq!(read_n(&mut inc, 4).await, b"ping");
    }

    #[tokio::test]
    async fn a_stream_opened_by_the_acceptor_carries_bytes_to_the_dialer() {
        let (dialer, acceptor) = connected().await;
        let mut out = open_stream(&acceptor).await;
        out.send.write_all(b"pong").await.expect("write to the stream");

        let mut inc = accept_stream(&dialer).await;

        assert_eq!(read_n(&mut inc, 4).await, b"pong");
    }
}

// ---- helpers ----

fn peer(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

/// A pair of ends of one line: `a` (node 1) looks at node 2, `b` (node 2) looks at node 1.
fn setup() -> (MemPath, MemPath) {
    let net = MemNet::new();
    MemPath::pair(&net.hub, (peer(1), 1), (peer(2), 2))
}

fn bytes(b: &[u8]) -> Bytes {
    Bytes::copy_from_slice(b)
}

fn hello(svc: ServiceId) -> Frame {
    Frame::Hello(bytes(svc.as_bytes()))
}

fn list_of(svcs: &[ServiceId]) -> Arc<[ServiceId]> {
    Arc::from(svcs)
}

/// A name with the same bytes but at a different memory address.
fn copy_of(svc: ServiceId) -> ServiceId {
    let leaked: &'static [u8] = Box::leak(svc.as_bytes().to_vec().into_boxed_slice());
    ServiceId::new(leaked)
}

fn info_of_family(family: &'static str) -> PathInfo {
    PathInfo {
        peer: peer(2),
        kind: PathKind::Direct,
        remote: None,
        family,
    }
}

async fn send_frame(p: &MemPath, frame: Frame) {
    p.send(frame.encode()).await.expect("send of the frame");
}

async fn recv_frame(p: &MemPath) -> Frame {
    let pkt = timeout(Duration::from_secs(5), p.recv()).await;
    let pkt = pkt.expect("recv hung").expect("recv returned an error");
    Frame::decode(pkt).expect("what arrived is not a frame")
}

async fn recv_err(p: &MemPath) -> ErrorKind {
    let got = timeout(Duration::from_secs(2), p.recv()).await;
    got.expect("recv hung")
        .expect_err("recv must return an error")
        .kind()
}

/// `start` under the common test time limit: a hung `start` does not hang the run.
async fn start_in_time(carrier: &MemCarrier, path: MemPath, role: CarrierRole) -> Started {
    let started = timeout(Duration::from_secs(5), carrier.start(Box::new(path), role)).await;
    started.expect("start did not return within 5 s")
}

/// Both ends with the real carrier: (dialer, acceptor).
async fn handshake(dial: ServiceId, list: &[ServiceId]) -> (Started, Started) {
    let (a, b) = setup();
    let carrier = MemCarrier::with_timeout(Duration::from_secs(5));
    let both = async {
        tokio::join!(
            carrier.start(Box::new(a), CarrierRole::Dial(dial)),
            carrier.start(Box::new(b), CarrierRole::Accept(list_of(list)))
        )
    };
    timeout(Duration::from_secs(10), both)
        .await
        .expect("the handshake hung")
}

async fn connected() -> (Arc<dyn PeerConn>, Arc<dyn PeerConn>) {
    let (dialed, accepted) = handshake(AUTH, &[AUTH]).await;
    let (_, dialer) = dialed.expect("the dialer must get a connection");
    let (_, acceptor) = accepted.expect("the acceptor must get a connection");
    (dialer, acceptor)
}

/// The dialer "by hand": Hello, Accept in reply and straight away a stream with data and an end.
async fn when_a_raw_dialer_opens_a_stream_at_once(b: &MemPath) {
    send_frame(b, hello(EXIT)).await;
    assert_eq!(recv_frame(b).await, Frame::Accept);
    send_frame(b, Frame::Open(1)).await;
    send_frame(b, Frame::Data(1, bytes(b"early"))).await;
    send_frame(b, Frame::Fin(1)).await;
}

fn error_of(started: Started) -> NetError {
    started.err().expect("start must return an error")
}

fn assert_refused(started: Started, who: &str) {
    let err = error_of(started);
    assert!(
        matches!(err, NetError::Refused),
        "{who}: expected Refused, got {err:?}"
    );
}

async fn open_stream(conn: &Arc<dyn PeerConn>) -> StreamPair {
    let opened = timeout(Duration::from_secs(5), conn.open()).await;
    opened.expect("open hung").expect("open returned an error")
}

async fn accept_stream(conn: &Arc<dyn PeerConn>) -> StreamPair {
    let accepted = timeout(Duration::from_secs(5), conn.accept()).await;
    accepted
        .expect("accept hung")
        .expect("accept returned an error")
}

async fn read_n(stream: &mut StreamPair, n: usize) -> Vec<u8> {
    let mut buf = vec![0u8; n];
    let read = timeout(Duration::from_secs(5), stream.recv.read_exact(&mut buf)).await;
    read.expect("read hung").expect("read failed");
    buf
}

async fn read_all(mut stream: StreamPair) -> Vec<u8> {
    let mut buf = Vec::new();
    let read = timeout(Duration::from_secs(5), stream.recv.read_to_end(&mut buf)).await;
    read.expect("read to the end hung")
        .expect("read failed");
    buf
}
