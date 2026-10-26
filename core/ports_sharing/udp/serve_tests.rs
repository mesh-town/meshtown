//! Tests of the publishing side of a UDP session (`serve_udp_stream`): frames from the stream become datagrams
//! to the local port, replies of that port become frames, a foreign sender is not heard, the loop ends with the stream.

use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{duplex, split, AsyncReadExt, AsyncWriteExt, DuplexStream};
use tokio::net::UdpSocket;
use tokio::task::JoinHandle;

use crate::ports_sharing::test_support::{datagram_frame, free_udp_port};
use crate::ports_sharing::udp::UdpSession;
use crate::transport::net::facade::test_support::within;

use super::*;

const IDLE: Duration = Duration::from_secs(60);
const QUIET: Duration = Duration::from_millis(300);

mod stream_frame {
    use super::*;

    #[tokio::test]
    async fn reaches_the_target_as_a_datagram() {
        // given: a session served towards a target socket
        let mut fx = setup().await;

        // when: the stream carries one frame
        fx.stream_says(&datagram_frame(b"ping")).await;

        // then: the target receives the body as one datagram
        let (body, _) = fx.target_hears().await;
        assert_eq!(body, b"ping");
    }
}

mod target_reply {
    use super::*;

    #[tokio::test]
    async fn forwards_target_reply_as_frame() {
        // given: a session; the target has heard a datagram and knows where it came from
        let mut fx = setup().await;
        fx.stream_says(&datagram_frame(b"ping")).await;
        let (_, from) = fx.target_hears().await;

        // when: the target answers
        fx.target.send_to(b"pong", from).await.unwrap();

        // then: the stream carries the answer as a frame
        assert_eq!(fx.stream_hears_frame().await, b"pong");
    }

    #[tokio::test]
    async fn datagram_of_2048_bytes_passes_whole() {
        assert_eq!(reply_of(2048).await, 2048);
    }

    #[tokio::test]
    async fn long_datagram_is_cut_to_2048() {
        assert_eq!(reply_of(2049).await, 2048);
        assert_eq!(reply_of(4000).await, 2048);
    }
}

mod foreign_sender {
    use super::*;

    #[tokio::test]
    async fn stranger_datagram_is_not_forwarded_to_the_stream() {
        // given: a session; the target knows the address of the session's socket
        let mut fx = setup().await;
        fx.stream_says(&datagram_frame(b"hello")).await;
        let (_, session_addr) = fx.target_hears().await;

        // when: a socket that is not the target sends to that address, then the target answers
        let stranger = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        stranger.send_to(b"forged", session_addr).await.unwrap();
        fx.target.send_to(b"real", session_addr).await.unwrap();

        // then: the stream carries the answer of the target and nothing before or after it
        assert_eq!(fx.stream_hears_frame().await, b"real");
        assert!(
            fx.stream_is_quiet().await,
            "a second frame reached the stream"
        );
    }
}

mod end_of_the_session {
    use super::*;

    #[tokio::test]
    async fn ends_on_stream_eof() {
        // given: a running session
        let mut fx = setup().await;

        // when: the remote side closes its half
        fx.stream.shutdown().await.unwrap();

        // then: the serving task ends
        let ended = within("serving ends", &mut fx.serving).await;
        assert!(ended.is_ok(), "the serving task panicked");
    }

    #[tokio::test]
    async fn goes_on_when_the_target_port_is_closed() {
        // given: a session towards a port nobody listens on
        let port = free_udp_port();
        let (mut stream, serving) = serve_towards(port);
        stream.write_all(&datagram_frame(b"lost")).await.unwrap();
        tokio::time::sleep(QUIET).await;

        // when: a socket appears on that port and the stream carries another frame
        // On some systems the refusal of the first datagram surfaces as a receive error; only the
        // failed send is logged and the loop continues.
        let target = UdpSocket::bind(("127.0.0.1", port)).await.unwrap();
        stream.write_all(&datagram_frame(b"found")).await.unwrap();

        // then: the new datagram arrives and the serving task is alive
        let mut buf = [0u8; 64];
        let (n, _) = within("target hears", target.recv_from(&mut buf))
            .await
            .unwrap();
        assert_eq!(&buf[..n], b"found");
        assert!(
            !serving.is_finished(),
            "the loop ended on a closed target port"
        );
    }
}

/// A session over a duplex pipe served towards `target`; the test holds the remote end of the pipe.
struct Fixture {
    stream: DuplexStream,
    target: UdpSocket,
    serving: JoinHandle<()>,
}

async fn setup() -> Fixture {
    let target = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let (stream, serving) = serve_towards(target.local_addr().unwrap().port());
    Fixture {
        stream,
        target,
        serving,
    }
}

/// Serves a session towards `port` over a duplex pipe; returns the remote end of the pipe and the serving task.
fn serve_towards(port: u16) -> (DuplexStream, JoinHandle<()>) {
    let (near, remote) = duplex(16 * 1024);
    let (recv, send) = split(near);
    let session = UdpSession::new(Box::new(send), Box::new(recv), IDLE, None);
    (remote, crate::node::output::spawn(serve_udp_stream(session, port)))
}

impl Fixture {
    async fn stream_says(&mut self, bytes: &[u8]) {
        self.stream.write_all(bytes).await.unwrap();
    }

    /// The body of the next frame in the stream.
    async fn stream_hears_frame(&mut self) -> Vec<u8> {
        let mut len = [0u8; 4];
        within("frame header", self.stream.read_exact(&mut len))
            .await
            .unwrap();
        let mut body = vec![0u8; u32::from_be_bytes(len) as usize];
        within("frame body", self.stream.read_exact(&mut body))
            .await
            .unwrap();
        body
    }

    /// Nothing more arrives in the stream for a short while.
    async fn stream_is_quiet(&mut self) -> bool {
        let mut b = [0u8; 1];
        tokio::time::timeout(QUIET, self.stream.read(&mut b))
            .await
            .is_err()
    }

    async fn target_hears(&self) -> (Vec<u8>, SocketAddr) {
        let mut buf = vec![0u8; 65_535];
        let got = within("target hears", self.target.recv_from(&mut buf))
            .await
            .unwrap();
        (buf[..got.0].to_vec(), got.1)
    }
}

/// The target answers with `len` bytes; returns the length of the body in the frame that reaches the stream.
async fn reply_of(len: usize) -> usize {
    let mut fx = setup().await;
    fx.stream_says(&datagram_frame(b"ping")).await;
    let (_, from) = fx.target_hears().await;
    fx.target.send_to(&vec![7u8; len], from).await.unwrap();
    fx.stream_hears_frame().await.len()
}
