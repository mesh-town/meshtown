//! A small HTTP/1.0 client for the node page on 127.0.0.1, shared by the CLI commands and the process host.
//! It has no TLS, redirects or retries. Calls block: async code runs them through `spawn_blocking`.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use crate::node::output::NET_NAME;

/// The identification probe is a courtesy to an error message: it must not hold the process up.
const PROBE_TIMEOUT: Duration = Duration::from_secs(1);

/// Why the call failed.
#[derive(Debug)]
pub enum LinkError {
    /// The connection was refused: no node is listening on this port.
    NotRunning,
    /// The socket failed; the error is kept so callers can tell a reset from a timeout.
    Io(std::io::Error),
    /// Something answered, but not with HTTP: it is not a node.
    NotNode,
}

/// Marks an error chain whose peer answered something that is not HTTP.
#[derive(Debug)]
pub struct NotNodeReply;

impl std::fmt::Display for NotNodeReply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{NET_NAME} did not reply over HTTP")
    }
}

impl std::error::Error for NotNodeReply {}

impl LinkError {
    /// The failure as an error value; the socket error stays in the chain, the text is in English.
    pub fn into_error(self) -> anyhow::Error {
        match self {
            LinkError::NotRunning => anyhow::anyhow!("{NET_NAME} is not running"),
            LinkError::Io(e) => anyhow::Error::new(e).context(format!("connection to {NET_NAME}")),
            LinkError::NotNode => anyhow::Error::new(NotNodeReply),
        }
    }
}

/// Whether a Mesh Town node answers on `127.0.0.1:<port>`: the open `GET /api/auth/status` (no token needed)
/// returns the JSON with the page's sign-in fields, which no other program serves.
pub fn node_page_answers(port: u16) -> bool {
    let reply = call_within(PROBE_TIMEOUT, port, "", "GET", "/api/auth/status", None);
    match reply {
        Ok((200, body)) => is_status_body(&body),
        _ => false,
    }
}

/// Whether the node on `127.0.0.1:<port>` accepts this permanent token: `GET /api/state` needs a credential,
/// so `200` means the node uses the same token file (the same settings directory). An empty token never matches.
pub fn node_accepts_token(port: u16, token: &str) -> bool {
    if token.is_empty() {
        return false;
    }
    let reply = call_within(PROBE_TIMEOUT, port, token, "GET", "/api/state", None);
    matches!(reply, Ok((200, _)))
}

fn is_status_body(body: &str) -> bool {
    let json: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
    ["password_set", "access", "listening"]
        .iter()
        .all(|key| json.get(key).is_some())
}

pub fn call_within(
    timeout: Duration,
    port: u16,
    token: &str,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<(u16, String), LinkError> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&addr, timeout).map_err(connect_error)?;
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));
    let request = request_text(port, token, method, path, body);
    stream
        .write_all(request.as_bytes())
        .map_err(link_error_from_io)?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).map_err(link_error_from_io)?;
    parse_response(&raw)
}

fn connect_error(e: std::io::Error) -> LinkError {
    match e.kind() {
        std::io::ErrorKind::ConnectionRefused => LinkError::NotRunning,
        _ => link_error_from_io(e),
    }
}

fn link_error_from_io(e: std::io::Error) -> LinkError {
    LinkError::Io(e)
}

fn request_text(port: u16, token: &str, method: &str, path: &str, body: Option<&str>) -> String {
    let body = body.unwrap_or("");
    let mut req = format!("{method} {path} HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\n");
    req.push_str(&format!(
        "Origin: http://127.0.0.1:{port}\r\nX-Meshtown-Token: {token}\r\n"
    ));
    if !body.is_empty() {
        req.push_str(&format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        ));
    }
    format!("{req}\r\n{body}")
}

fn parse_response(raw: &[u8]) -> Result<(u16, String), LinkError> {
    let text = String::from_utf8_lossy(raw);
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    let code = head
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse::<u16>().ok());
    match code {
        Some(code) => Ok((code, body.to_string())),
        None => Err(LinkError::NotNode),
    }
}

#[cfg(test)]
#[path = "probe_tests.rs"]
mod tests;
