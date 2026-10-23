//! The proxy auto-config file (PAC): pure functions — recognize the request, build the script and the reply.
//! I/O and the listener port are in `http.rs`; the script text is in `pac.js` next to it.

use std::net::SocketAddr;

/// The PAC path on the proxy port.
pub(super) const PAC_PATH: &str = "/proxy.pac";
/// The script with the `__ROUTE__` and `__MESH__` markers in place of the route strings.
const PAC_TEMPLATE: &str = include_str!("pac.js");

#[derive(Debug, PartialEq, Eq)]
pub(super) struct PacRequest {
    /// `HEAD`: headers without a body.
    pub head_only: bool,
}

/// `Some` for `GET`/`HEAD` on `/proxy.pac` or `/proxy.pac?…`, otherwise `None`.
pub(super) fn pac_request(method: &str, path: &str) -> Option<PacRequest> {
    let head_only = match method {
        "GET" => false,
        "HEAD" => true,
        _ => return None,
    };
    let rest = path.strip_prefix(PAC_PATH)?;
    if !rest.is_empty() && !rest.starts_with('?') {
        return None;
    }
    Some(PacRequest { head_only })
}

/// The route string for the browser: `DIRECT` or a node (SOCKS5, then HTTP proxy; without a `DIRECT` tail).
pub(super) fn pac_proxy_list(proxy: SocketAddr, direct_only: bool) -> String {
    if direct_only {
        return "DIRECT".to_string();
    }
    format!("SOCKS5 {proxy}; PROXY {proxy}")
}

/// The PAC script: `route` for ordinary hosts, `mesh` for mesh names (`meshtown`, `*.meshtown`).
pub(super) fn pac_script(route: &str, mesh: &str) -> String {
    PAC_TEMPLATE
        .replace("__MESH__", mesh)
        .replace("__ROUTE__", route)
}

/// The whole reply; `HEAD` has no body, `Content-Length` is the same.
pub(super) fn pac_reply(script: &str, head_only: bool) -> Vec<u8> {
    let mut out = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/x-ns-proxy-autoconfig\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
        script.len()
    )
    .into_bytes();
    if !head_only {
        out.extend_from_slice(script.as_bytes());
    }
    out
}

/// The PAC address on the proxy port.
pub fn pac_url(proxy: SocketAddr) -> String {
    format!("http://{proxy}{PAC_PATH}")
}

#[cfg(test)]
#[path = "pac_tests.rs"]
mod tests;
