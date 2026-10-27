//! The proxy auto-config file (PAC): pure functions — recognize the request, build the script and the reply.
//! I/O and the listener port are in `http.rs`; the script text is in `pac.js` next to it.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};

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

/// The route string for mesh names: a node (SOCKS5, then HTTP proxy), never a `DIRECT` tail.
pub(super) fn pac_proxy_list(proxy: SocketAddr) -> String {
    format!("SOCKS5 {proxy}; PROXY {proxy}")
}

/// The route string for ordinary hosts: the node, plus a `DIRECT` tail unless `strict`.
pub(super) fn pac_route_list(proxy: SocketAddr, strict: bool) -> String {
    let list = pac_proxy_list(proxy);
    if strict {
        return list;
    }
    format!("{list}; DIRECT")
}

/// Strict mode: ordinary hosts have no `DIRECT` fallback when the node does not answer.
static PAC_STRICT: AtomicBool = AtomicBool::new(false);

/// Sets strict mode for every PAC served from now on.
pub fn set_pac_strict(on: bool) {
    PAC_STRICT.store(on, Ordering::Relaxed);
}

/// Whether the PAC currently omits the `DIRECT` fallback.
pub fn pac_strict() -> bool {
    PAC_STRICT.load(Ordering::Relaxed)
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
