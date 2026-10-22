//! Transport: accepting a connection, body size limit, chunked refusal, request parsing.

pub(crate) const MAX_REQUEST_HEAD_BYTES: usize = 24 * 1024;
pub(crate) const MAX_REQUEST_BODY_BYTES: usize = 256 * 1024;
pub(crate) const HEADER_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// The `Host` literal from the socket address and port: on 80 without a port.
pub(crate) fn host_of(ip: std::net::IpAddr, port: u16) -> String {
    match port {
        80 => ip.to_string(),
        _ => format!("{ip}:{port}"),
    }
}

/// Socket addresses of a connection: who came and to which local address. Put into the extensions of every
/// request; the guard checks the peer and `Host` against it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ConnInfo {
    pub client_addr: std::net::SocketAddr,
    pub local: std::net::SocketAddr,
}

#[cfg(test)]
impl ConnInfo {
    /// Loopback `127.0.0.1:50000 → 127.0.0.1:port`.
    pub(crate) fn loopback(port: u16) -> ConnInfo {
        let ip = std::net::Ipv4Addr::LOCALHOST;
        ConnInfo {
            client_addr: (ip, 50000).into(),
            local: (ip, port).into(),
        }
    }
}

/// Accepts one already open TCP connection and serves it through `router` (shared by all
/// connections, cheap to clone). Request head limits and its read timer are here;
/// overflowing `max_buf_size` drops the connection without a reply, `max_headers` gives an empty 431.
/// `conn` is put into the extensions of every request.
pub(crate) async fn serve_connection(
    stream: tokio::net::TcpStream,
    conn: ConnInfo,
    router: axum::Router,
) {
    let io = hyper_util::rt::TokioIo::new(stream);
    let svc = tower::service_fn(move |req: hyper::Request<hyper::body::Incoming>| {
        let mut router = router.clone();
        async move {
            let mut req: axum::extract::Request = req.map(axum::body::Body::new);
            req.extensions_mut().insert(conn);
            tower::Service::<axum::extract::Request>::call(&mut router, req).await
        }
    });
    let hyper_svc = hyper_util::service::TowerToHyperService::new(svc);

    let mut builder = hyper::server::conn::http1::Builder::new();
    builder
        .max_buf_size(MAX_REQUEST_HEAD_BYTES)
        .max_headers(64)
        .keep_alive(true)
        .timer(hyper_util::rt::TokioTimer::new())
        .header_read_timeout(HEADER_READ_TIMEOUT);
    let _ = builder.serve_connection(io, hyper_svc).await;
}

pub(crate) async fn body_limit(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use http_body_util::{BodyExt, Limited};
    let (parts, body) = req.into_parts();
    let limited = Limited::new(body, MAX_REQUEST_BODY_BYTES);
    let bytes = match limited.collect().await {
        Ok(c) => c.to_bytes(),
        Err(_) => {
            return json_err(
                axum::http::StatusCode::PAYLOAD_TOO_LARGE,
                "too_large",
                "body larger than 256 KiB",
            )
        }
    };
    next.run(axum::extract::Request::from_parts(
        parts,
        axum::body::Body::from(bytes),
    ))
    .await
}

pub(crate) async fn reject_chunked(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    if req
        .headers()
        .get(axum::http::header::TRANSFER_ENCODING)
        .is_some()
    {
        let mut resp = json_err(
            axum::http::StatusCode::BAD_REQUEST,
            "bad_request",
            "chunked isn't supported",
        );
        resp.headers_mut().insert(
            axum::http::header::CONNECTION,
            axum::http::HeaderValue::from_static("close"),
        );
        return resp;
    }
    next.run(req).await
}

pub(crate) fn json_err(
    status: axum::http::StatusCode,
    code: &str,
    message: &str,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let body = serde_json::json!({"error": code, "message": message});
    (status, axum::Json(body)).into_response()
}

/// The value of the first query-string parameter matched by name, with `application/
/// x-www-form-urlencoded` (`+` → space, `%XX` → byte) decoding; an invalid escape sequence
/// is left as is, the first occurrence of the name wins.
pub(crate) fn query_param(uri: &axum::http::Uri, name: &str) -> Option<String> {
    let query = uri.query()?;
    for pair in query.split('&') {
        let mut it = pair.splitn(2, '=');
        let key = it.next().unwrap_or("");
        let value = it.next().unwrap_or("");
        if decode_form_urlencoded(key) == name {
            return Some(decode_form_urlencoded(value));
        }
    }
    None
}

fn decode_form_urlencoded(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len()
                && bytes[i + 1].is_ascii_hexdigit()
                && bytes[i + 2].is_ascii_hexdigit() =>
            {
                let hi = (bytes[i + 1] as char).to_digit(16).unwrap();
                let lo = (bytes[i + 2] as char).to_digit(16).unwrap();
                out.push((hi * 16 + lo) as u8);
                i += 3;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;
