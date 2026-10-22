//! `connect` under its own timeout, then one `write_all` and a read under a separate timeout.
//! The behaviour is checked through `get_state`/`post_stop` — the tests in `client_tests.rs`, section `raw_*`.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

use super::response::{parse_response, MAX_BODY_BYTES, MAX_HEADER_BYTES};
use super::types::ApiError;

/// The timeout for establishing the TCP connection.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
/// The timeout for sending the request and receiving the whole response, separate from `CONNECT_TIMEOUT`.
const EXCHANGE_TIMEOUT: Duration = Duration::from_secs(2);

/// The exchange timeout for the invite: the node may take long to assemble the reply (longer than `EXCHANGE_TIMEOUT`).
pub(crate) const INVITE_EXCHANGE_TIMEOUT: Duration = Duration::from_secs(5);

/// `connect` under its own timeout, then one `write_all` and a read under a separate timeout. Any
/// error or timeout of `connect` → `Refused`; an exchange timeout → `Timeout`; closed or reset before the first
/// byte of the response → `NotHttp("connection closed without a reply")`.
pub(crate) async fn exchange(request: &[u8], port: u16) -> Result<(u16, Vec<u8>), ApiError> {
    exchange_within(request, port, EXCHANGE_TIMEOUT).await
}

/// The same as `exchange`, but with its own exchange timeout `limit`; the `connect` timeout stays the same.
pub(crate) async fn exchange_within(
    request: &[u8],
    port: u16,
    limit: Duration,
) -> Result<(u16, Vec<u8>), ApiError> {
    let mut stream = match timeout(CONNECT_TIMEOUT, TcpStream::connect(("127.0.0.1", port))).await {
        Ok(Ok(stream)) => stream,
        _ => return Err(ApiError::Refused),
    };

    match timeout(limit, send_request_read_response(&mut stream, request)).await {
        Ok(result) => result,
        Err(_) => Err(ApiError::Timeout),
    }
}

/// Writes the request, reads in chunks and calls `parse_response` after each one to find out whether the
/// body is complete; an error from an intermediate (necessarily incomplete) call is silently dropped — only
/// the last call (after a close or a reset) or an explicit success counts.
async fn send_request_read_response(
    stream: &mut TcpStream,
    request: &[u8],
) -> Result<(u16, Vec<u8>), ApiError> {
    let closed_without_reply =
        || ApiError::NotHttp("connection closed without a reply".to_string());

    if stream.write_all(request).await.is_err() {
        return Err(closed_without_reply());
    }

    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        if let Ok(parsed) = parse_response(&buf) {
            return Ok(parsed);
        }
        if buf.len() > MAX_HEADER_BYTES + MAX_BODY_BYTES {
            return parse_response(&buf);
        }
        match stream.read(&mut chunk).await {
            Ok(0) => {
                return if buf.is_empty() {
                    Err(closed_without_reply())
                } else {
                    parse_response(&buf)
                };
            }
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(_) => {
                return if buf.is_empty() {
                    Err(closed_without_reply())
                } else {
                    parse_response(&buf)
                };
            }
        }
    }
}
