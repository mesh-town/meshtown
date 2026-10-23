use super::*;
use crate::proxy::mesh::page::{render_page, MeshPage, PageKind};
use crate::transport::net::{BoxRecv, BoxSend};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::Instant;

const GET_HEAD: &[u8] = b"GET / HTTP/1.1\r\nHost: x.meshtown\r\n\r\n";
const HEAD_HEAD: &[u8] = b"HEAD / HTTP/1.1\r\nHost: x.meshtown\r\n\r\n";
const HEAD_LIMIT: usize = 16 * 1024;

mod request_head {
    use super::*;

    mod get {
        use super::*;

        #[tokio::test(start_paused = true)]
        async fn is_answered_with_the_full_page_then_eof() {
            // given
            let mut client = setup();

            // when
            client.write(GET_HEAD).await;
            let answer = client.read_until_eof().await;

            // then
            assert_eq!(answer, render_page(&page(), false));
        }

        #[tokio::test(start_paused = true)]
        async fn answer_starts_with_a_status_line_and_has_a_body() {
            // given
            let mut client = setup();

            // when
            client.write(GET_HEAD).await;
            let answer = client.read_until_eof().await;

            // then
            assert!(answer.starts_with(b"HTTP/1.1 "), "not an HTTP response");
            let body = body_of(&answer);
            assert!(!body.is_empty(), "GET answer must carry the page body");
        }

        #[tokio::test(start_paused = true)]
        async fn head_arriving_in_two_pieces_is_answered_once_complete() {
            // given
            let mut client = setup();

            // when
            client.write(b"GET / HTTP/1.1\r\nHo").await;
            tokio::time::sleep(Duration::from_secs(1)).await;
            client.write(b"st: x.meshtown\r\n\r\n").await;
            let answer = client.read_until_eof().await;

            // then
            assert_eq!(answer, render_page(&page(), false));
        }
    }

    mod head {
        use super::*;

        #[tokio::test(start_paused = true)]
        async fn is_answered_with_the_page_head_and_no_body() {
            // given
            let mut client = setup();

            // when
            client.write(HEAD_HEAD).await;
            let answer = client.read_until_eof().await;

            // then
            assert_eq!(answer, render_page(&page(), true));
            assert!(body_of(&answer).is_empty(), "HEAD answer must have no body");
        }

        #[tokio::test(start_paused = true)]
        async fn announces_the_same_length_as_get() {
            // given
            let get = render_page(&page(), false);
            let head = render_page(&page(), true);

            // then
            assert_eq!(
                content_length(&head),
                content_length(&get),
                "HEAD must announce the GET body length"
            );
            assert!(head.len() < get.len());
        }
    }

    mod tls_client_hello {
        use super::*;

        #[tokio::test(start_paused = true)]
        async fn first_byte_0x16_closes_without_a_single_byte() {
            // given
            let mut client = setup();

            // when
            client.write(&[0x16, 0x03, 0x01, 0x00, 0x05, 0x01, 0x00]).await;
            let answer = client.read_until_eof().await;

            // then
            assert!(answer.is_empty(), "TLS hello got {} bytes", answer.len());
        }

        #[tokio::test(start_paused = true)]
        async fn closes_without_waiting_for_the_head_timeout() {
            // given
            let mut client = setup();
            let start = Instant::now();

            // when
            client.write(&[0x16]).await;
            client.read_until_eof().await;

            // then
            assert!(
                start.elapsed() < PAGE_HEAD_TIMEOUT,
                "TLS hello must be refused at once, took {:?}",
                start.elapsed()
            );
        }
    }

    mod silence {
        use super::*;

        #[test]
        fn timeout_is_five_seconds() {
            assert_eq!(PAGE_HEAD_TIMEOUT, Duration::from_secs(5));
        }

        #[tokio::test(start_paused = true)]
        async fn closes_without_bytes_after_the_head_timeout() {
            // given
            let mut client = setup();
            let start = Instant::now();

            // when
            let answer = client.read_until_eof().await;

            // then
            assert!(answer.is_empty(), "silent client got {} bytes", answer.len());
            assert!(
                start.elapsed() >= PAGE_HEAD_TIMEOUT,
                "closed too early: {:?}",
                start.elapsed()
            );
        }

        #[tokio::test(start_paused = true)]
        async fn stays_open_until_the_head_timeout() {
            // given
            let mut client = setup();

            // when
            tokio::time::sleep(PAGE_HEAD_TIMEOUT - Duration::from_secs(1)).await;

            // then
            assert!(client.nothing_to_read_yet().await, "closed before the timeout");
        }

        #[tokio::test(start_paused = true)]
        async fn unfinished_head_is_closed_after_the_head_timeout() {
            // given
            let mut client = setup();
            let start = Instant::now();

            // when
            client.write(b"GET / HTTP/1.1\r\nHost: x.mesh").await;
            let answer = client.read_until_eof().await;

            // then
            assert!(answer.is_empty(), "partial head got {} bytes", answer.len());
            assert!(start.elapsed() >= PAGE_HEAD_TIMEOUT);
        }
    }

    mod endless_head {
        use super::*;

        #[tokio::test(start_paused = true)]
        async fn sixteen_kib_without_the_end_closes_without_bytes() {
            // given
            let mut client = setup();
            let start = Instant::now();

            // when
            client.write_in_background(endless_head(HEAD_LIMIT + 1024));
            let answer = client.read_until_eof().await;

            // then
            assert!(answer.is_empty(), "oversized head got {} bytes", answer.len());
            assert!(
                start.elapsed() < PAGE_HEAD_TIMEOUT,
                "limit must close before the timeout, took {:?}",
                start.elapsed()
            );
        }
    }
}

struct Client {
    send: Option<BoxSend>,
    recv: BoxRecv,
}

impl Client {
    async fn write(&mut self, bytes: &[u8]) {
        let send = self.send.as_mut().expect("send half taken");
        // The server may already have closed; a failed write is not the point of a test.
        let _ = send.write_all(bytes).await;
        let _ = send.flush().await;
    }

    fn write_in_background(&mut self, bytes: Vec<u8>) {
        let mut send = self.send.take().expect("send half taken");
        tokio::spawn(async move {
            let _ = send.write_all(&bytes).await;
            let _ = send.flush().await;
            // Keep the half open: the server must stop on its own.
            std::future::pending::<()>().await;
        });
    }

    async fn read_until_eof(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        let _ = self.recv.read_to_end(&mut out).await;
        out
    }

    async fn nothing_to_read_yet(&mut self) -> bool {
        let mut byte = [0u8; 1];
        let read = tokio::time::timeout(Duration::from_millis(10), self.recv.read(&mut byte));
        read.await.is_err()
    }
}

fn setup() -> Client {
    let (send, recv) = serve_page(page());
    Client {
        send: Some(send),
        recv,
    }
}

fn page() -> MeshPage {
    MeshPage {
        me: "Cafe Router".to_string(),
        host: "x.meshtown".to_string(),
        kind: PageKind::NotFound {
            label: "x".to_string(),
            suggest: vec![],
        },
    }
}

fn endless_head(len: usize) -> Vec<u8> {
    let mut head = b"GET / HTTP/1.1\r\nX-Pad: ".to_vec();
    head.resize(len, b'a');
    head
}

fn body_of(response: &[u8]) -> &[u8] {
    let end = response
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("response has no end of head");
    &response[end + 4..]
}

fn content_length(response: &[u8]) -> usize {
    let end = response.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8_lossy(&response[..end]).to_ascii_lowercase();
    head.lines()
        .find_map(|l| l.strip_prefix("content-length:"))
        .expect("no Content-Length header")
        .trim()
        .parse()
        .expect("Content-Length is not a number")
}
