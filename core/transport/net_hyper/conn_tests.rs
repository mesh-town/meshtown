//! A session over the control stream: opening streams, closing, silence.
//! No DHT: the control stream is `tokio::io::duplex`, the "network" is a fake `StreamDialer`.

use super::*;
use crate::transport::net::id::NodeSecret;
use crate::transport::net_hyper::wire::{
    read_ctrl, read_header, read_reply, write_ctrl, write_reply,
};
use crate::transport::net_hyper::wire::{Ctrl, Header, Owner, Reply};
use crate::transport::net_hyper::wire::{REFUSE_NO_SESSION, REFUSE_UNSERVED};
use std::io::ErrorKind;
use std::sync::Mutex;
use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};
use tokio::time::{sleep, timeout};

const SESSION: u64 = 0xABCD_0001;
const WITHIN: Duration = Duration::from_secs(2);

fn peer(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

/// A fake "network" on behalf of `me`: a data stream goes straight into the `remote` table.
struct PairDialer {
    me: PeerId,
    remote: Arc<Sessions>,
    refuse: Option<u32>,
    seen: Arc<Mutex<Vec<Header>>>,
}

impl PairDialer {
    fn new(me: PeerId, remote: Arc<Sessions>) -> Arc<Self> {
        Arc::new(Self {
            me,
            remote,
            refuse: None,
            seen: Arc::default(),
        })
    }

    fn refusing(me: PeerId, remote: Arc<Sessions>, code: u32) -> Arc<Self> {
        Arc::new(Self {
            me,
            remote,
            refuse: Some(code),
            seen: Arc::default(),
        })
    }

    fn headers(&self) -> Vec<Header> {
        self.seen.lock().unwrap().clone()
    }
}

impl StreamDialer for PairDialer {
    fn dial(&self, _to: PeerId) -> BoxFuture<'_, NetResult<Box<dyn Duplex>>> {
        Box::pin(async move {
            let (near, far) = duplex(64 * 1024);
            crate::node::output::spawn(far_side(
                far,
                self.me,
                self.remote.clone(),
                self.refuse,
                self.seen.clone(),
            ));
            Ok(Box::new(near) as Box<dyn Duplex>)
        })
    }
}

/// The role of the accepting transport: header, reply, route into the table.
async fn far_side(
    mut far: tokio::io::DuplexStream,
    from: PeerId,
    table: Arc<Sessions>,
    refuse: Option<u32>,
    seen: Arc<Mutex<Vec<Header>>>,
) {
    let Ok(header) = read_header(&mut far).await else {
        return;
    };
    seen.lock().unwrap().push(header.clone());
    let Header::Stream { session, owner } = header else {
        return;
    };
    if let Some(code) = refuse {
        let _ = write_reply(
            &mut far,
            &Reply::Refuse {
                code,
                reason: b"no".to_vec(),
            },
        )
        .await;
        return;
    }
    let _ = write_reply(&mut far, &Reply::Ack).await;
    let (r, w) = tokio::io::split(far);
    let key = SessionKey {
        peer: from,
        session,
        numbered_by_us: owner == Owner::Acceptor,
    };
    let _ = route(
        &table,
        key,
        StreamPair {
            send: Box::new(w),
            recv: Box::new(r),
        },
    );
}

/// A helper in place of the removed `Sessions::route`: a queue slot, then sending the stream.
fn route(table: &Sessions, key: SessionKey, pair: StreamPair) -> Result<(), u32> {
    let permit = table.reserve(key)?;
    permit.send(pair);
    Ok(())
}

struct Two {
    a: Arc<HyperConn>,
    b: Arc<HyperConn>,
    dial_a: Arc<PairDialer>,
    dial_b: Arc<PairDialer>,
}

/// A dialed, B accepted; each has its own table, the "network" leads to the other side's table.
async fn two() -> Two {
    let (id_a, id_b) = (peer(1), peer(2));
    let (table_a, table_b) = (Arc::new(Sessions::new()), Arc::new(Sessions::new()));
    let dial_a = PairDialer::new(id_a, table_b.clone());
    let dial_b = PairDialer::new(id_b, table_a.clone());
    let (ca, mut cb) = duplex(8192);
    let accept = async {
        let header = read_header(&mut cb).await.unwrap();
        assert_eq!(
            header,
            Header::Open {
                session: SESSION,
                service_name: b"svc".to_vec()
            }
        );
        establish_accepted_session(id_a, SESSION, Box::new(cb), dial_b.clone(), table_b.clone())
            .await
    };
    let dial = establish_dialed_session(
        id_b,
        SESSION,
        b"svc",
        Box::new(ca),
        dial_a.clone(),
        table_a.clone(),
    );
    let (a, b) = tokio::join!(dial, accept);
    Two {
        a: a.unwrap(),
        b: b.unwrap(),
        dial_a,
        dial_b,
    }
}

async fn within<T>(fut: impl std::future::Future<Output = T>) -> T {
    timeout(WITHIN, fut)
        .await
        .expect("did not finish within 2 s")
}

#[tokio::test]
async fn open_sends_stream_header_and_returns_after_ack() {
    let t = two().await;
    let mut mine = within(t.a.open()).await.unwrap();
    let mut theirs = within(t.b.accept()).await.unwrap();
    mine.send.write_all(b"hello").await.unwrap();
    let mut got = [0u8; 5];
    theirs.recv.read_exact(&mut got).await.unwrap();
    assert_eq!(&got, b"hello");
    assert_eq!(
        t.dial_a.headers(),
        vec![Header::Stream {
            session: SESSION,
            owner: Owner::Dialer
        }]
    );
    assert_eq!(t.a.stable_id(), SESSION);
    assert_eq!(t.b.stable_id(), SESSION);
}

#[tokio::test]
async fn open_refused_by_peer_is_an_error() {
    let (id_a, id_b) = (peer(1), peer(2));
    let (table_a, table_b) = (Arc::new(Sessions::new()), Arc::new(Sessions::new()));
    let refusing = PairDialer::refusing(id_a, table_b, REFUSE_NO_SESSION);
    let (ca, mut cb) = duplex(8192);
    let far = crate::node::output::spawn(async move {
        read_header(&mut cb).await.unwrap();
        write_reply(&mut cb, &Reply::Ack).await.unwrap();
        cb
    });
    let a = establish_dialed_session(id_b, SESSION, b"svc", Box::new(ca), refusing, table_a)
        .await
        .unwrap();
    let _keep = far.await.unwrap();
    assert!(matches!(within(a.open()).await, Err(NetError::Closed)));
}

#[tokio::test]
async fn unserved_refusal_maps_to_refused() {
    let (id_a, id_b) = (peer(1), peer(2));
    let (ca, mut cb) = duplex(8192);
    let far = crate::node::output::spawn(async move {
        read_header(&mut cb).await.unwrap();
        write_reply(
            &mut cb,
            &Reply::Refuse {
                code: REFUSE_UNSERVED,
                reason: b"x".to_vec(),
            },
        )
        .await
        .unwrap();
    });
    let dial = PairDialer::new(id_a, Arc::new(Sessions::new()));
    let res = establish_dialed_session(
        id_b,
        SESSION,
        b"svc",
        Box::new(ca),
        dial,
        Arc::new(Sessions::new()),
    )
    .await;
    far.await.unwrap();
    assert!(matches!(res, Err(NetError::Refused)));
}

#[tokio::test]
async fn acceptor_side_open_reaches_dialer_accept() {
    let t = two().await;
    let mut from_b = within(t.b.open()).await.unwrap();
    let mut at_a = within(t.a.accept()).await.unwrap();
    from_b.send.write_all(b"back").await.unwrap();
    let mut got = [0u8; 4];
    at_a.recv.read_exact(&mut got).await.unwrap();
    assert_eq!(&got, b"back");
    assert_eq!(
        t.dial_b.headers(),
        vec![Header::Stream {
            session: SESSION,
            owner: Owner::Acceptor
        }]
    );
}

#[tokio::test]
async fn stream_right_after_ack_finds_the_session() {
    let table = Arc::new(Sessions::new());
    let (ca, mut cb) = duplex(8192);
    let dial = PairDialer::new(peer(2), Arc::new(Sessions::new()));
    let acc = crate::node::output::spawn(establish_accepted_session(
        peer(1),
        SESSION,
        Box::new(ca),
        dial,
        table.clone(),
    ));
    assert_eq!(read_reply(&mut cb).await.unwrap(), Reply::Ack);
    let (x, _y) = duplex(64);
    let (r, w) = tokio::io::split(x);
    let key = SessionKey {
        peer: peer(1),
        session: SESSION,
        numbered_by_us: false,
    };
    assert_eq!(
        route(
            &table,
            key,
            StreamPair {
                send: Box::new(w),
                recv: Box::new(r)
            }
        ),
        Ok(())
    );
    let _conn = acc.await.unwrap().unwrap();
}

#[tokio::test]
async fn dialer_registers_before_it_sends_open() {
    let table = Arc::new(Sessions::new());
    let (ca, mut cb) = duplex(8192);
    let dial = PairDialer::new(peer(1), Arc::new(Sessions::new()));
    let task = crate::node::output::spawn(establish_dialed_session(
        peer(2),
        SESSION,
        b"svc",
        Box::new(ca),
        dial,
        table.clone(),
    ));
    read_header(&mut cb).await.unwrap();
    let (x, _y) = duplex(64);
    let (r, w) = tokio::io::split(x);
    let key = SessionKey {
        peer: peer(2),
        session: SESSION,
        numbered_by_us: true,
    };
    assert_eq!(
        route(
            &table,
            key,
            StreamPair {
                send: Box::new(w),
                recv: Box::new(r)
            }
        ),
        Ok(())
    );
    write_reply(&mut cb, &Reply::Ack).await.unwrap();
    let _conn = task.await.unwrap().unwrap();
}

#[tokio::test]
async fn close_sends_close_frame_and_peer_sees_closed() {
    let t = two().await;
    t.a.close(7, b"bye");
    assert!(t.a.is_closed());
    within(t.b.closed()).await;
    assert!(t.b.is_closed());
    within(t.a.closed()).await;
}

#[tokio::test]
async fn close_reason_names_code_and_reason() {
    let t = two().await;
    assert_eq!(t.a.close_reason(), "");
    t.a.close(7, b"bye");
    assert_eq!(t.a.close_reason(), "code 7: bye");
    within(t.b.closed()).await;
    assert_eq!(t.b.close_reason(), "code 7: bye");
}

#[tokio::test]
async fn dropping_last_handle_closes_with_code_zero() {
    let t = two().await;
    drop(t.a);
    within(t.b.closed()).await;
    assert!(
        t.b.close_reason().starts_with("code 0"),
        "{}",
        t.b.close_reason()
    );
}

/// A peer that answered `Ack` and hands the control stream to the test.
async fn dialed_against_raw_peer() -> (Arc<HyperConn>, tokio::io::DuplexStream) {
    let (ca, mut cb) = duplex(8192);
    let dial = PairDialer::new(peer(1), Arc::new(Sessions::new()));
    let task = crate::node::output::spawn(establish_dialed_session(
        peer(2),
        SESSION,
        b"svc",
        Box::new(ca),
        dial,
        Arc::new(Sessions::new()),
    ));
    read_header(&mut cb).await.unwrap();
    write_reply(&mut cb, &Reply::Ack).await.unwrap();
    (task.await.unwrap().unwrap(), cb)
}

#[tokio::test]
async fn peer_hangup_reason_is_read_ended() {
    let (conn, far) = dialed_against_raw_peer().await;
    drop(far);
    within(conn.closed()).await;
    assert_eq!(conn.close_reason(), "code 0: read ended");
}

#[tokio::test(start_paused = true)]
async fn silence_longer_than_dead_after_closes() {
    let (conn, _far) = dialed_against_raw_peer().await;
    sleep(DEAD_AFTER - Duration::from_secs(1)).await;
    assert!(
        !conn.is_closed(),
        "too early: silence is shorter than DEAD_AFTER"
    );
    sleep(Duration::from_secs(2)).await;
    assert!(
        conn.is_closed(),
        "silence longer than DEAD_AFTER did not close the session"
    );
}

#[tokio::test(start_paused = true)]
async fn pong_to_every_ping_keeps_the_session_alive() {
    let (conn, mut far) = dialed_against_raw_peer().await;
    let echo = crate::node::output::spawn(async move {
        while let Ok(Ctrl::Ping) = read_ctrl(&mut far).await {
            write_ctrl(&mut far, &Ctrl::Pong).await.unwrap();
        }
    });
    sleep(DEAD_AFTER * 3).await;
    assert!(!conn.is_closed());
    echo.abort();
}

#[tokio::test]
async fn accept_after_close_is_closed() {
    let t = two().await;
    t.a.close(0, b"");
    assert!(matches!(within(t.a.accept()).await, Err(NetError::Closed)));
    within(t.b.closed()).await;
    assert!(matches!(within(t.b.accept()).await, Err(NetError::Closed)));
}

#[tokio::test]
async fn stream_of_closed_session_fails_on_next_write() {
    let t = two().await;
    let mut mine = within(t.a.open()).await.unwrap();
    let _theirs = within(t.b.accept()).await.unwrap();
    t.a.close(0, b"");
    let err = mine.send.write_all(b"x").await.unwrap_err();
    assert_eq!(err.kind(), ErrorKind::ConnectionAborted);
}

#[tokio::test]
async fn open_on_closed_session_is_closed() {
    let t = two().await;
    t.a.close(0, b"");
    assert!(matches!(t.a.open().await, Err(NetError::Closed)));
}

#[tokio::test]
async fn accepted_stream_is_counted_and_aborts_after_close() {
    let t = two().await;
    let mut from_b = within(t.b.open()).await.unwrap();
    let mut at_a = within(t.a.accept()).await.unwrap();
    from_b.send.write_all(b"ping").await.unwrap();
    let mut got = [0u8; 4];
    at_a.recv.read_exact(&mut got).await.unwrap();
    at_a.send.write_all(b"pong!").await.unwrap();
    let info = t.a.link();
    assert_eq!((info.recv_bytes, info.sent_bytes), (4, 5));
    t.a.close(0, b"");
    let err = at_a.send.write_all(b"x").await.unwrap_err();
    assert_eq!(err.kind(), ErrorKind::ConnectionAborted);
    let err = at_a.recv.read(&mut got).await.unwrap_err();
    assert_eq!(err.kind(), ErrorKind::ConnectionAborted);
}
