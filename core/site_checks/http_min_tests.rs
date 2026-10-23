//! Tests for the minimal HTTP client and TLS config (`http_min.rs`).

use super::*;
use tokio::io::duplex;

/// Runs one request through a pair of in-memory streams: the server returns pre-set bytes.
async fn round(reply: &'static [u8], close: bool) -> (HttpResponse, Vec<u8>) {
    let (mut client, mut server) = duplex(128 * 1024);
    let task = crate::transport::net::task_label::spawn(async move {
        let mut got = vec![0u8; 2048];
        let n = server.read(&mut got).await.unwrap();
        got.truncate(n);
        server.write_all(reply).await.unwrap();
        if close {
            server.shutdown().await.unwrap();
        } else {
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        got
    });
    let limits = HttpLimits {
        connect: Duration::from_secs(1),
        read: Duration::from_millis(300),
        total: Duration::from_secs(3),
        max_bytes: 64 * 1024,
    };
    let req = HttpGetRequest {
        host: "example.com",
        path: "/",
        accept: "*/*",
        user_agent: "meshtown-test",
    };
    let resp = http_get(&mut client, req, limits).await.unwrap();
    let sent = if close {
        task.await.unwrap()
    } else {
        task.abort();
        Vec::new()
    };
    (resp, sent)
}

#[tokio::test]
async fn request_has_host_and_close() {
    let (_, sent) = round(
        b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n",
        true,
    )
    .await;
    let text = String::from_utf8_lossy(&sent).to_string();
    assert!(text.starts_with("GET / HTTP/1.1\r\n"), "{text}");
    assert!(text.contains("Host: example.com\r\n"), "{text}");
    assert!(text.contains("Connection: close\r\n"), "{text}");
    assert!(text.contains("Accept-Encoding: identity\r\n"), "{text}");
    assert!(!text.to_ascii_lowercase().contains("@"), "{text}");
}

#[tokio::test]
async fn parses_status_and_headers() {
    let (resp, _) = round(
        b"HTTP/1.1 451 Unavailable\r\nContent-Length: 2\r\nX-Note: Yes\r\n\r\nhi",
        true,
    )
    .await;
    assert_eq!(resp.status, 451);
    assert_eq!(resp.header("x-note"), Some("Yes"));
    assert_eq!(resp.header("X-NOTE"), Some("Yes"));
    assert!(!resp.stalled);
}

#[tokio::test]
async fn body_by_content_length() {
    let (resp, _) = round(
        b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello-and-more",
        true,
    )
    .await;
    assert_eq!(resp.body, b"hello");
    assert!(resp.raw_bytes > resp.body.len());
}

#[tokio::test]
async fn body_chunked() {
    let (resp, _) = round(
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n3\r\n it\r\n0\r\n\r\n",
        true,
    )
    .await;
    assert_eq!(resp.body, b"hello it");
}

#[tokio::test]
async fn stall_is_marked_and_bytes_counted() {
    let mut reply = Vec::from(&b"HTTP/1.1 200 OK\r\nContent-Length: 1000000\r\n\r\n"[..]);
    reply.extend(std::iter::repeat_n(b'x', 20 * 1024));
    let reply: &'static [u8] = Vec::leak(reply);
    let (resp, _) = round(reply, false).await;
    assert!(resp.stalled);
    assert!(!resp.eof);
    assert!(resp.raw_bytes >= 20 * 1024, "{}", resp.raw_bytes);
    assert!(resp.raw_bytes < 22 * 1024, "{}", resp.raw_bytes);
}
