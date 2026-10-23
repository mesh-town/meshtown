//! Tests for `http.rs`: request body limit, chunked refusal, JSON error, query parameter parsing.

use super::*;
use axum::body::Body;
use axum::http::{HeaderValue, Request, StatusCode};
use axum::middleware;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::Router;
use http_body_util::BodyExt;
use tower::ServiceExt;

async fn echo_ok(_req: axum::extract::Request) -> axum::response::Response {
    (StatusCode::OK, "ok").into_response()
}

fn body_limit_router() -> Router {
    Router::new()
        .route("/x", post(echo_ok))
        .layer(middleware::from_fn(body_limit))
}

fn reject_chunked_router() -> Router {
    Router::new()
        .route("/x", post(echo_ok))
        .layer(middleware::from_fn(reject_chunked))
}

#[tokio::test]
async fn body_limit_rejects_over_256_kib() {
    let big = vec![b'a'; 300 * 1024];
    let req = Request::builder()
        .method("POST")
        .uri("/x")
        .body(Body::from(big))
        .unwrap();
    let resp = body_limit_router().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["error"], "too_large");
}

#[tokio::test]
async fn body_limit_passes_small_body() {
    let small = vec![b'a'; 100];
    let req = Request::builder()
        .method("POST")
        .uri("/x")
        .body(Body::from(small))
        .unwrap();
    let resp = body_limit_router().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn reject_chunked_rejects_transfer_encoding() {
    let req = Request::builder()
        .method("POST")
        .uri("/x")
        .header("transfer-encoding", "chunked")
        .body(Body::empty())
        .unwrap();
    let resp = reject_chunked_router().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        resp.headers().get(axum::http::header::CONNECTION),
        Some(&HeaderValue::from_static("close"))
    );
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["error"], "bad_request");
    assert_eq!(v["message"], "chunked isn't supported");
}

#[tokio::test]
async fn reject_chunked_passes_without_header() {
    let req = Request::builder()
        .method("POST")
        .uri("/x")
        .body(Body::empty())
        .unwrap();
    let resp = reject_chunked_router().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn json_err_has_json_content_type_and_body() {
    let resp = json_err(StatusCode::FORBIDDEN, "bad_host", "foreign Host");
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        resp.headers().get(axum::http::header::CONTENT_TYPE),
        Some(&HeaderValue::from_static("application/json"))
    );
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["error"], "bad_host");
    assert_eq!(v["message"], "foreign Host");
}

#[test]
fn query_param_decodes_plus_and_percent() {
    let uri: axum::http::Uri = "/api/log?token=a%2Bb+c&after=5".parse().unwrap();
    assert_eq!(query_param(&uri, "token"), Some("a+b c".to_string()));
    assert_eq!(query_param(&uri, "after"), Some("5".to_string()));
}

#[test]
fn query_param_missing_is_none() {
    let uri: axum::http::Uri = "/api/log?after=5".parse().unwrap();
    assert_eq!(query_param(&uri, "token"), None);
}

#[test]
fn query_param_first_occurrence_wins() {
    let uri: axum::http::Uri = "/api/domains?name=a&name=b".parse().unwrap();
    assert_eq!(query_param(&uri, "name"), Some("a".to_string()));
}

mod host_of {
    use super::super::host_of;
    use std::net::{IpAddr, Ipv4Addr};

    const IP: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 5));

    #[test]
    fn port_80_bare() {
        assert_eq!(host_of(IP, 80), "192.168.1.5");
    }

    #[test]
    fn other_port_with_colon() {
        assert_eq!(host_of(IP, 7070), "192.168.1.5:7070");
    }
}

mod serve_connection {
    use super::super::{serve_connection, ConnInfo};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn peer_of(axum::extract::Extension(conn): axum::extract::Extension<ConnInfo>) -> String {
        format!("{}|{}", conn.client_addr, conn.local)
    }

    #[tokio::test]
    async fn puts_conn_info_into_extensions() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        let router = axum::Router::new().route("/", axum::routing::get(peer_of));
        let server = crate::node::output::spawn(async move {
            let (stream, peer) = listener.accept().await.unwrap();
            let conn = ConnInfo {
                client_addr: peer,
                local: stream.local_addr().unwrap(),
            };
            serve_connection(stream, conn, router).await;
        });
        let mut client = tokio::net::TcpStream::connect(addr).await.unwrap();
        let from = client.local_addr().unwrap();
        client
            .write_all(b"GET / HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let mut out = String::new();
        client.read_to_string(&mut out).await.unwrap();
        server.await.unwrap();
        assert!(out.contains(&format!("{from}|{addr}")), "{out}");
    }
}
