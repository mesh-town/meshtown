use super::*;
use crate::exit::udpgw::test_support::{cfg_with, spawn_echo, start, Client, WAIT};
use tokio::time::timeout;

fn to_domain(conn_id: u16, name: &str, data: &[u8]) -> Frame {
    Frame {
        flags: FLAG_DATA,
        conn_id,
        target: Some(Target::Domain(name.into(), 9)),
        data: data.to_vec(),
    }
}

async fn fixture(limits: Limits) -> (Client, JoinHandle<()>, SocketAddr) {
    let echo = spawn_echo().await;
    let cfg = cfg_with(HashMap::from([("echo.test".to_string(), echo)]));
    let (client, task) = start(cfg, limits);
    (client, task, echo)
}

const PLAIN: Limits = LIMITS;

#[tokio::test]
async fn datagram_goes_out_and_the_reply_comes_back_with_the_same_conn_id() {
    let (mut client, _task, echo) = fixture(PLAIN).await;
    client.send(&to_domain(5, "echo.test", b"ping")).await;
    let reply = client.next().await;
    assert_eq!(reply, Frame::data(5, echo, b"ping".to_vec()));
}

#[tokio::test]
async fn two_conn_ids_use_two_sockets() {
    let (mut client, _task, _echo) = fixture(PLAIN).await;
    client.send(&to_domain(1, "echo.test", b"a")).await;
    client.send(&to_domain(2, "echo.test", b"b")).await;
    let mut got = [client.next().await, client.next().await];
    got.sort_by_key(|f| f.conn_id);
    assert_eq!((got[0].conn_id, got[0].data.clone()), (1, b"a".to_vec()));
    assert_eq!((got[1].conn_id, got[1].data.clone()), (2, b"b".to_vec()));
}

#[tokio::test]
async fn closed_destination_is_dropped_without_a_reply() {
    let (mut client, _task, echo) = fixture(PLAIN).await;
    // the same loopback echo, but named by a literal address: the guard refuses it
    client.send(&Frame::data(1, echo, b"x".to_vec())).await;
    assert!(client.silent().await);
}

#[tokio::test]
async fn keepalive_is_answered_with_keepalive() {
    let (mut client, _task, _echo) = fixture(PLAIN).await;
    client.send(&Frame::keepalive(3)).await;
    assert_eq!(client.next().await, Frame::keepalive(3));
}

#[tokio::test]
async fn frame_with_only_unknown_flags_is_answered_with_an_error_frame() {
    let (mut client, _task, _echo) = fixture(PLAIN).await;
    for (flags, conn_id) in [(0x40, 1), (0x00, 2), (0x08, 3), (0x20, 4)] {
        client
            .send(&Frame {
                flags,
                conn_id,
                target: None,
                data: vec![],
            })
            .await;
        assert_eq!(client.next().await, Frame::error(conn_id));
    }
    client.send(&Frame::keepalive(1)).await;
    assert_eq!(client.next().await.flags, FLAG_KEEPALIVE);
}

#[tokio::test]
async fn sessions_over_the_limit_are_dropped_until_one_is_closed() {
    let limits = Limits {
        idle: Duration::from_secs(60),
        max_sessions: 2,
    };
    let (mut client, _task, _echo) = fixture(limits).await;
    client.send(&to_domain(1, "echo.test", b"a")).await;
    client.send(&to_domain(2, "echo.test", b"b")).await;
    client.next().await;
    client.next().await;
    client.send(&to_domain(3, "echo.test", b"c")).await;
    assert!(client.silent().await);
    client
        .send(&Frame {
            flags: FLAG_CLOSE,
            conn_id: 1,
            target: None,
            data: vec![],
        })
        .await;
    client.send(&to_domain(3, "echo.test", b"c")).await;
    assert_eq!(client.next().await.conn_id, 3);
}

#[tokio::test]
async fn idle_session_is_released() {
    let limits = Limits {
        idle: Duration::from_millis(200),
        max_sessions: 1,
    };
    let (mut client, _task, _echo) = fixture(limits).await;
    client.send(&to_domain(1, "echo.test", b"a")).await;
    client.next().await;
    tokio::time::sleep(Duration::from_millis(700)).await;
    client.send(&to_domain(2, "echo.test", b"b")).await;
    assert_eq!(client.next().await.conn_id, 2);
}

#[tokio::test]
async fn closing_the_stream_ends_the_loop() {
    let (client, task, _echo) = fixture(PLAIN).await;
    drop(client);
    timeout(WAIT, task)
        .await
        .expect("loop ended")
        .expect("no panic");
}
