use super::*;
use std::io::{Read, Write};

#[test]
fn request_has_host_origin_and_token() {
    let r = request_text(7777, "T", "PUT", "/api/config", Some("{}"));
    assert!(r.starts_with("PUT /api/config HTTP/1.0\r\n"), "{r}");
    assert!(r.contains("Host: 127.0.0.1:7777\r\n"), "{r}");
    assert!(r.contains("Origin: http://127.0.0.1:7777\r\n"), "{r}");
    assert!(r.contains("X-Meshtown-Token: T\r\n"), "{r}");
    assert!(r.ends_with("\r\n\r\n{}"), "{r}");
}

#[test]
fn response_gives_code_and_body() {
    let (code, body) = parse_response(b"HTTP/1.0 403 Forbidden\r\nA: b\r\n\r\n{\"x\":1}").unwrap();
    assert_eq!((code, body.as_str()), (403, "{\"x\":1}"));
}

#[test]
fn garbage_is_other_error() {
    match parse_response(b"hello") {
        Err(LinkError::Other(text)) => assert_eq!(text, "Mesh Town did not reply over HTTP"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn closed_port_is_not_running() {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    drop(l);
    // Windows retries a refused connection to localhost for about two seconds before reporting it.
    assert!(matches!(
        call_within(Duration::from_secs(5), port, "T", "GET", "/", None),
        Err(LinkError::NotRunning)
    ));
}

#[test]
fn status_body_needs_the_sign_in_fields() {
    assert!(is_status_body(
        r#"{"password_set":true,"access":"localhost","listening":"localhost","via":null}"#
    ));
    assert!(!is_status_body(r#"{"access":"x"}"#));
    assert!(!is_status_body("<html></html>"));
}

#[test]
fn silent_listener_is_not_a_node() {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    assert!(!node_page_answers(port));
}

#[test]
fn listener_with_status_json_is_a_node() {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (mut conn, _) = l.accept().unwrap();
        let mut buf = [0u8; 1024];
        let _ = conn.read(&mut buf);
        let body =
            r#"{"password_set":false,"access":"localhost","listening":"localhost","via":null}"#;
        let _ = write!(conn, "HTTP/1.0 200 OK\r\n\r\n{body}");
    });
    assert!(node_page_answers(port));
    server.join().unwrap();
}
