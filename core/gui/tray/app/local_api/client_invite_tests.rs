//! Timeouts of the exchange with the node: an invite request waits for the reply up to `INVITE_EXCHANGE_TIMEOUT`
//! (5 s), the client's other requests keep the previous 2 s. The fake node is its own `TcpListener` on 127.0.0.1,
//! as in the `raw_*` section of `client_tests.rs`. The time is real: with tokio's clock paused and
//! a real socket the timer can fire before the kernel delivers the bytes, and the test turns red
//! for no reason. "Not earlier" is checked strictly, the upper bounds leave headroom for machine load.

use super::*;

use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

use super::super::transport::{exchange_within, INVITE_EXCHANGE_TIMEOUT};
use crate::gui::tray::app::local_api::tests::FAKE_TOKEN;

/// The node's response body for `GET /api/network/invite`.
const BODY: &str = r#"{"invite":"mesh-invite2-FAKE","holders":2,"reach":"spread"}"#;
/// The node answers after this time: longer than the usual 2 s, shorter than the invite's 5 s limit.
const SLOW_ANSWER: Duration = Duration::from_millis(3500);

mod invite_exchange {
    use super::*;

    mod node_answers_after_three_and_a_half_seconds {
        use super::*;

        #[tokio::test(flavor = "multi_thread")]
        async fn get_invite_waits_and_gets_the_answer() {
            let node = setup(Node::AnswersAfter(SLOW_ANSWER)).await;

            let reply = get_invite(node.port, FAKE_TOKEN).await;

            assert!(
                matches!(reply, Ok(Reply::Ok(_))),
                "get_invite must wait for a reply that arrives after 3.5 s (limit 5 s)"
            );
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn get_state_on_the_same_node_gives_up_after_two_seconds() {
            let node = setup(Node::AnswersAfter(SLOW_ANSWER)).await;
            let started = Instant::now();

            let reply = get_state(node.port, FAKE_TOKEN).await;

            assert!(
                matches!(reply, Err(ApiError::Timeout)),
                "get_state stays on the previous 2 s and does not wait for a reply after 3.5 s"
            );
            assert!(
                started.elapsed() >= Duration::from_secs(2),
                "timeout earlier than 2 s"
            );
        }
    }

    mod silent_node {
        use super::*;

        #[tokio::test(flavor = "multi_thread")]
        async fn get_invite_times_out_not_earlier_than_five_seconds() {
            let node = setup(Node::Silent).await;
            let started = Instant::now();

            let reply = get_invite(node.port, FAKE_TOKEN).await;

            let waited = started.elapsed();
            assert!(
                matches!(reply, Err(ApiError::Timeout)),
                "a silent node — Timeout"
            );
            assert!(
                waited >= Duration::from_secs(5),
                "invite timeout earlier than 5 s"
            );
            assert!(
                waited <= Duration::from_secs(8),
                "invite timeout later than 8 s"
            );
        }
    }

    mod exchange_limit {
        use super::*;

        #[test]
        fn invite_limit_is_five_seconds() {
            assert_eq!(INVITE_EXCHANGE_TIMEOUT, Duration::from_secs(5));
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn answer_later_than_limit_is_timeout() {
            let node = setup(Node::AnswersAfter(Duration::from_millis(600))).await;

            let result = when_exchanging(&node, Duration::from_millis(200)).await;

            assert!(
                matches!(result, Err(ApiError::Timeout)),
                "a reply after the limit — Timeout"
            );
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn answer_earlier_than_limit_comes_through() {
            let node = setup(Node::AnswersAfter(Duration::from_millis(300))).await;

            let result = when_exchanging(&node, Duration::from_secs(2)).await;

            let (status, body) = result.expect("a reply within the limit must arrive");
            assert_eq!(status, 200);
            assert_eq!(body, BODY.as_bytes());
        }
    }
}

/// How the stub node behaves after it has read the request.
#[derive(Clone, Copy)]
enum Node {
    /// Never answers.
    Silent,
    /// Answers HTTP 200 with `BODY` after the given time.
    AnswersAfter(Duration),
}

/// A running stub: the port and the task accepting connections (shut down on `Drop`).
struct StubNode {
    port: u16,
    accept_loop: JoinHandle<()>,
}

impl Drop for StubNode {
    fn drop(&mut self) {
        self.accept_loop.abort();
    }
}

/// Precondition: a node on 127.0.0.1, a free port, behaviour `node`.
async fn setup(node: Node) -> StubNode {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("local_addr").port();
    let accept_loop = tokio::spawn(async move {
        while let Ok((socket, _addr)) = listener.accept().await {
            tokio::spawn(serve(socket, node));
        }
    });
    StubNode { port, accept_loop }
}

/// One connection: reads the request, then stays silent or answers according to `node`; keeps the connection open
/// until the end of the test runtime.
async fn serve(mut socket: TcpStream, node: Node) {
    let mut request = [0u8; 4096];
    let _ = socket.read(&mut request).await;
    if let Node::AnswersAfter(delay) = node {
        tokio::time::sleep(delay).await;
        let _ = socket.write_all(http_200(BODY).as_bytes()).await;
    }
    std::future::pending::<()>().await;
}

fn http_200(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// An invite request straight through `exchange_within` with its own limit.
async fn when_exchanging(node: &StubNode, limit: Duration) -> Result<(u16, Vec<u8>), ApiError> {
    let request = request_bytes_without_body("GET", "/api/network/invite", node.port, FAKE_TOKEN);
    exchange_within(&request, node.port, limit).await
}
