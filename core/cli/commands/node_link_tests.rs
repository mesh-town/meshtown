use super::*;

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
    assert!(matches!(parse_response(b"hello"), Err(LinkError::Other(_))));
}

#[test]
fn closed_port_is_not_running() {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    drop(l);
    assert!(matches!(
        call(port, "T", "GET", "/", None),
        Err(LinkError::NotRunning)
    ));
}
