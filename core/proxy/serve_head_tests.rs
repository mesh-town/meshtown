//! Tests for the deadline on the request head (`serve.rs`, `http.rs`): a connection that has not delivered
//! its head by the deadline is closed silently; once the head is parsed the deadline no longer applies.

use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout, Instant};

use super::*;
use crate::exit::test_support::Scripted;
use crate::proxy::path::busy_tests::config_with;

const DEADLINE: Duration = Duration::from_millis(200);
/// The outer limit of a "closed at the deadline" test: five deadlines.
const OUTER: Duration = Duration::from_millis(1000);
const SLACK: Duration = Duration::from_millis(50);
const GREETING: [u8; 3] = [5, 1, 0];
const SOCKS_ASSOCIATE: [u8; 10] = [5, 3, 0, 1, 0, 0, 0, 0, 0, 0];

/// A `TcpStream` pair on loopback: the end accepted by the server and the client's end.
async fn loopback_pair() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let peer = TcpStream::connect(listener.local_addr().unwrap())
        .await
        .unwrap();
    let (accepted, _) = listener.accept().await.unwrap();
    (accepted, peer)
}

/// Runs `dispatch` on a fresh connection with the deadline `limit` from now; returns the client's end and the task.
async fn start(cfg: Option<ProxyConfig>, limit: Duration) -> (TcpStream, JoinHandle<()>) {
    let (accepted, client) = loopback_pair().await;
    let client_addr = client.local_addr().unwrap();
    let deadline = tokio::time::Instant::now() + limit;
    let task = crate::node::output::spawn(dispatch(
        accepted,
        cfg,
        NetDirectory::default(),
        client_addr,
        deadline,
    ));
    (client, task)
}

/// Reads until the end of the stream (a reset counts as the end); returns the bytes and the time it took.
/// Fails if the stream does not end within `OUTER`.
async fn read_until_closed<R: AsyncReadExt + Unpin>(client: &mut R) -> (Vec<u8>, Duration) {
    let began = Instant::now();
    let mut got = Vec::new();
    let mut chunk = [0u8; 256];
    let reading = async {
        loop {
            match client.read(&mut chunk).await {
                Ok(n) if n > 0 => got.extend_from_slice(&chunk[..n]),
                _ => break,
            }
        }
    };
    timeout(OUTER, reading)
        .await
        .expect("the connection was not closed in time");
    (got, began.elapsed())
}

fn assert_not_before_deadline(took: Duration) {
    assert!(
        took + SLACK >= DEADLINE,
        "closed after {took:?}, before the deadline"
    );
}

fn connect_request(port: u16) -> Vec<u8> {
    let mut request = vec![5, 1, 0, 1, 127, 0, 0, 1];
    request.extend_from_slice(&port.to_be_bytes());
    request
}

/// A TCP echo server on loopback; returns its port.
async fn echo_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    crate::node::output::spawn(async move {
        while let Ok((mut conn, _)) = listener.accept().await {
            crate::node::output::spawn(async move {
                let mut buf = [0u8; 256];
                while let Ok(n) = conn.read(&mut buf).await {
                    if n == 0 || conn.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
    port
}

/// A direct-route config (no exits are chosen).
async fn direct_config() -> ProxyConfig {
    config_with(&[], || Arc::new(Scripted::new(b""))).await
}

/// Echoes `ping` back through a tunnel after a pause of two deadlines.
async fn assert_echo_after_silence(client: &mut TcpStream) {
    sleep(DEADLINE * 2).await;
    client.write_all(b"ping").await.unwrap();
    let mut back = [0u8; 4];
    timeout(OUTER, client.read_exact(&mut back))
        .await
        .expect("no echo")
        .unwrap();
    assert_eq!(&back, b"ping");
}

mod before_the_head {
    use super::*;

    #[tokio::test]
    async fn untouched_socket_is_closed_at_the_deadline() {
        let (mut client, _task) = start(None, DEADLINE).await;

        let (got, took) = read_until_closed(&mut client).await;

        assert!(got.is_empty());
        assert_not_before_deadline(took);
    }

    #[tokio::test]
    async fn client_closing_untouched_socket_ends_dispatch() {
        let (client, task) = start(None, Duration::from_secs(5)).await;
        drop(client);

        let ended = timeout(Duration::from_secs(1), task).await;

        assert!(ended.is_ok(), "dispatch waited for the deadline");
    }

    #[tokio::test]
    async fn http_partial_head_is_closed_at_the_deadline() {
        let (mut client, _task) = start(None, DEADLINE).await;
        client
            .write_all(b"GET http://site.test/ HTTP/1.1\r\nHost: si")
            .await
            .unwrap();

        let (got, took) = read_until_closed(&mut client).await;

        assert!(got.is_empty(), "unexpected reply: {got:?}");
        assert_not_before_deadline(took);
    }

    #[tokio::test]
    async fn head_trickling_past_the_deadline_is_closed() {
        let (mut client, _task) = start(None, DEADLINE).await;
        client
            .write_all(b"GET /proxy.pac HTTP/1.1\r\nX: ")
            .await
            .unwrap();
        let (mut reader, mut writer) = client.into_split();
        let dripper = crate::node::output::spawn(async move {
            while writer.write_all(b"a").await.is_ok() {
                sleep(Duration::from_millis(70)).await;
            }
        });

        let (_got, took) = read_until_closed(&mut reader).await;
        dripper.abort();

        assert_not_before_deadline(took);
    }

    #[tokio::test]
    async fn http_head_in_two_writes_before_deadline_is_served() {
        let (mut client, _task) = start(None, DEADLINE).await;
        client
            .write_all(b"GET /proxy.pac HTTP/1.1\r\nHost: x\r\n")
            .await
            .unwrap();
        sleep(Duration::from_millis(50)).await;
        client.write_all(b"\r\n").await.unwrap();

        let (got, _) = read_until_closed(&mut client).await;

        assert!(got.starts_with(b"HTTP/1.1 200"), "reply: {got:?}");
    }

    #[tokio::test]
    async fn socks_partial_greeting_is_closed_at_the_deadline() {
        let (mut client, _task) = start(None, DEADLINE).await;
        client.write_all(&[5]).await.unwrap();

        let (got, took) = read_until_closed(&mut client).await;

        assert!(got.is_empty(), "unexpected reply: {got:?}");
        assert_not_before_deadline(took);
    }

    #[tokio::test]
    async fn socks_greeting_without_request_is_closed_at_the_deadline() {
        let (mut client, _task) = start(None, DEADLINE).await;
        client.write_all(&GREETING).await.unwrap();

        let (got, took) = read_until_closed(&mut client).await;

        assert_eq!(got, vec![5, 0]);
        assert_not_before_deadline(took);
    }

    #[tokio::test]
    async fn socks_request_before_deadline_gets_a_reply() {
        let (mut client, _task) = start(None, DEADLINE).await;
        let mut hello = GREETING.to_vec();
        hello.extend(connect_request(9));
        client.write_all(&hello).await.unwrap();

        let (got, _) = read_until_closed(&mut client).await;

        assert_eq!(&got[..2], &[5, 0]);
        assert_eq!(got[3], socks5::REP_HOST_UNREACHABLE, "reply: {got:?}");
    }
}

mod after_the_head {
    use super::*;

    #[tokio::test]
    async fn socks_connect_tunnel_outlives_the_head_deadline() {
        let echo = echo_server().await;
        let (mut client, _task) = start(Some(direct_config().await), DEADLINE).await;
        let mut hello = GREETING.to_vec();
        hello.extend(connect_request(echo));
        client.write_all(&hello).await.unwrap();
        let mut replies = [0u8; 12];
        timeout(OUTER, client.read_exact(&mut replies))
            .await
            .expect("no reply")
            .unwrap();
        assert_eq!(replies[3], socks5::REP_OK, "reply: {replies:?}");

        assert_echo_after_silence(&mut client).await;
    }

    #[tokio::test]
    async fn http_connect_tunnel_outlives_the_head_deadline() {
        let echo = echo_server().await;
        let (mut client, _task) = start(Some(direct_config().await), DEADLINE).await;
        let head = format!("CONNECT 127.0.0.1:{echo} HTTP/1.1\r\nHost: 127.0.0.1:{echo}\r\n\r\n");
        client.write_all(head.as_bytes()).await.unwrap();
        let mut answer = Vec::new();
        while !answer.ends_with(b"\r\n\r\n") {
            let mut byte = [0u8; 1];
            timeout(OUTER, client.read_exact(&mut byte))
                .await
                .expect("no reply")
                .unwrap();
            answer.push(byte[0]);
        }
        assert!(answer.starts_with(b"HTTP/1.1 200"), "reply: {answer:?}");

        assert_echo_after_silence(&mut client).await;
    }

    #[tokio::test]
    async fn associate_control_outlives_the_head_deadline() {
        let (mut client, _task) = start(Some(direct_config().await), DEADLINE).await;
        let mut hello = GREETING.to_vec();
        hello.extend_from_slice(&SOCKS_ASSOCIATE);
        client.write_all(&hello).await.unwrap();
        let mut replies = [0u8; 12];
        timeout(OUTER, client.read_exact(&mut replies))
            .await
            .expect("no reply")
            .unwrap();
        assert_eq!(replies[3], socks5::REP_OK, "reply: {replies:?}");

        sleep(DEADLINE * 2).await;

        let mut byte = [0u8; 1];
        let read = timeout(Duration::from_millis(150), client.read(&mut byte)).await;
        assert!(read.is_err(), "the control connection was closed: {read:?}");
    }
}
