use super::*;

// ==============================================================================================
// request_bytes_* (4)
// ==============================================================================================

#[test]
fn request_bytes_get_matches_spec_bytes() {
    let bytes = request_bytes_without_body("GET", "/api/state", 7777, "TOK1234");
    assert_eq!(
        bytes,
        b"GET /api/state HTTP/1.0\r\nHost: 127.0.0.1:7777\r\nX-Meshtown-Token: TOK1234\r\n\
          Accept: application/json\r\n\r\n"
            .to_vec()
    );
}

#[test]
fn request_bytes_post_matches_spec_bytes() {
    let bytes = request_bytes_without_body("POST", "/api/actions/stop", 7777, "TOK1234");
    assert_eq!(
        bytes,
        b"POST /api/actions/stop HTTP/1.0\r\nHost: 127.0.0.1:7777\r\n\
          Origin: http://127.0.0.1:7777\r\nX-Meshtown-Token: TOK1234\r\nContent-Length: 0\r\n\r\n"
            .to_vec()
    );
}

#[test]
fn request_bytes_port_80_omits_port_in_host_and_origin() {
    let get = request_bytes_without_body("GET", "/app.js", 80, "TOK1234");
    assert_eq!(
        get,
        b"GET /app.js HTTP/1.0\r\nHost: 127.0.0.1\r\nX-Meshtown-Token: TOK1234\r\n\
          Accept: application/json\r\n\r\n"
            .to_vec()
    );
    let post = request_bytes_without_body("POST", "/api/actions/stop", 80, "TOK1234");
    assert_eq!(
        post,
        b"POST /api/actions/stop HTTP/1.0\r\nHost: 127.0.0.1\r\nOrigin: http://127.0.0.1\r\n\
          X-Meshtown-Token: TOK1234\r\nContent-Length: 0\r\n\r\n"
            .to_vec()
    );
}

#[test]
fn request_bytes_token_with_crlf_is_omitted() {
    let bytes = request_bytes_without_body("GET", "/api/state", 7777, "bad\r\ntoken");
    assert_eq!(
        bytes,
        b"GET /api/state HTTP/1.0\r\nHost: 127.0.0.1:7777\r\nAccept: application/json\r\n\r\n"
            .to_vec()
    );
}
