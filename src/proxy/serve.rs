//! The proxy's listening socket (SOCKS5 and HTTP on one port) and the SOCKS5 "face": parsing the request and
//! replying to the client; the route to the target is in `path`, HTTP is in `http`.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use tokio::net::{TcpListener, TcpStream};

use crate::config_file::ListenAccess;
use crate::doh::Resolver;
use crate::exit::PeerConnPool;
use crate::net::NetStack;
use crate::output::say;
use crate::route::RouteCtx;
use crate::socks5;
use crate::tcp_tune::disable_nagle;
use crate::web::auth::access::bind_ip;
use crate::web::auth::iface_os::current_view;

use super::admit::{admit_conn, iface_deny_text, is_proxy_client_allowed, Admission, DenyLog};
use super::outcome::{outcome_line, ProxyOutcome};
use super::{http, path};

/// The default port of the local SOCKS5 server.
pub const DEFAULT_SOCKS_PORT: u16 = 1080;
pub const DIRECT_TCP_CONNECT_TIMEOUT: Duration = Duration::from_secs(8);

/// Everything the SOCKS5 server needs to work.
#[derive(Clone)]
pub struct ProxyConfig {
    pub route: RouteCtx,
    pub stack: NetStack,
    pub pool: PeerConnPool,
    pub resolver: Arc<Resolver>,
    /// Domain addresses that bypass DNS: always empty in the node, filled only by tests.
    pub host_overrides: Arc<HashMap<String, SocketAddr>>,
}

/// Why the proxy port could not be taken: the port, the kind of error and the OS text. It is printed for a human:
/// the first line is `Display`, the second (the advice) is `advice`.
#[derive(Debug)]
pub struct BindError {
    pub port: u16,
    pub io_error_kind: std::io::ErrorKind,
    pub os_message: String,
}

impl std::fmt::Display for BindError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (port, os_message) = (self.port, &self.os_message);
        match self.io_error_kind {
            std::io::ErrorKind::AddrInUse => {
                write!(
                    f,
                    "port {port} for the browser proxy is in use by another program ({os_message})."
                )
            }
            std::io::ErrorKind::PermissionDenied => {
                write!(
                    f,
                    "no permission to open port {port} for the browser proxy ({os_message})."
                )
            }
            _ => write!(
                f,
                "port {port} for the browser proxy is in use or unavailable ({os_message})."
            ),
        }
    }
}

impl std::error::Error for BindError {}

impl BindError {
    /// The second line of the message: what to do. The other port is the neighboring one, below by one for 65535.
    pub fn advice(&self) -> String {
        let other = if self.port == u16::MAX {
            self.port - 1
        } else {
            self.port + 1
        };
        let cmd = format!(
            "{} run --socks-port {other}",
            crate::program_name::program_name()
        );
        match self.io_error_kind {
            std::io::ErrorKind::AddrInUse => {
                format!("Close it or start the node on another port: {cmd}")
            }
            _ => format!("Start the node on another port: {cmd}"),
        }
    }
}

/// The proxy bind address: `localhost` is `127.0.0.1`, `localnet` is `0.0.0.0` (IPv4 only, like the page).
pub fn listen_ip(access: ListenAccess) -> std::net::Ipv4Addr {
    bind_ip(access)
}

/// Takes the port: `localhost` on `127.0.0.1`, `localnet` on `0.0.0.0` (the admission by client address
/// is in `serve_proxy`). A busy port is an error, the node does not start.
pub async fn bind_proxy_listener(port: u16, access: ListenAccess) -> Result<TcpListener> {
    let addr = SocketAddr::from((listen_ip(access), port));
    TcpListener::bind(addr).await.map_err(|e| {
        anyhow::Error::new(BindError {
            port,
            io_error_kind: e.kind(),
            os_message: e.to_string(),
        })
    })
}

/// Accepts connections while the listening socket is alive; foreign ones are closed at once, without a reply, by `access` and `ifaces`.
pub async fn serve_proxy(listener: TcpListener, cfg: ProxyConfig, adm: Admission) {
    let mut deny_log = DenyLog::default();
    loop {
        let (stream, client_addr) = match listener.accept().await {
            Ok(pair) => pair,
            Err(e) => {
                say("socks", format!("accepting connections stopped: {e}"));
                return;
            }
        };
        if !is_proxy_client_allowed(adm.access, client_addr.ip()) {
            deny_log.log_address_deny(client_addr);
            continue;
        }
        let verdict = admit_conn(&adm, &current_view(), client_addr, stream.local_addr().ok());
        if !verdict.allows() {
            deny_log.log_iface_deny(client_addr, &iface_deny_text(client_addr, &verdict, &adm));
            continue;
        }
        disable_nagle(&stream);
        let cfg = cfg.clone();
        tokio::spawn(dispatch(stream, cfg, client_addr));
    }
}

/// Tells the protocol apart by the first byte without taking it from the stream: `0x05` is SOCKS5,
/// otherwise HTTP; a connection closed without a single byte is a quiet exit. It prints the error with the
/// protocol label.
async fn dispatch(stream: TcpStream, cfg: ProxyConfig, client_addr: SocketAddr) {
    let mut first = [0u8; 1];
    match stream.peek(&mut first).await {
        Ok(n) if n > 0 => {}
        _ => return,
    }
    if first[0] == socks5::VERSION {
        if let Err(e) = serve_socks5_connection(stream, cfg).await {
            say("socks", format!("{client_addr}: {e:#}"));
        }
        return;
    }
    if let Err(e) = http::serve_http_connection(stream, cfg).await {
        say("http", format!("{client_addr}: {e:#}"));
    }
}

/// Serves one SOCKS5 connection from the greeting to the end of the transfer. There is one route decision for
/// the whole connection and it does not depend on the target (a domain or an address);
/// a connection through an exit is closed if the exit is removed from `route.exits` while it is still open.
async fn serve_socks5_connection(mut stream: TcpStream, cfg: ProxyConfig) -> Result<()> {
    // One timestamp for the whole serving: the summary line prints it.
    let started = Instant::now();
    socks5::read_greeting(&mut stream).await?;
    let raw = socks5::read_request_bytes(&mut stream).await?;
    let request = match socks5::parse_socks5_request(&raw) {
        Ok(request) => request,
        Err(e) => {
            // The request was not parsed — there is no target, nothing to write a summary line about.
            say("socks", format!("request rejected: {e}"));
            socks5::write_reply(&mut stream, e.reply_code()).await?;
            return Ok(());
        }
    };
    match request.command {
        socks5::Command::Connect => serve_connect(stream, cfg, request, started).await,
        socks5::Command::UdpAssociate => refuse_associate(stream).await,
    }
}

/// `UDP ASSOCIATE` is not served yet: the client gets "command not supported".
async fn refuse_associate(mut stream: TcpStream) -> Result<()> {
    say("socks", "udp associate: not supported".to_string());
    socks5::write_reply(&mut stream, socks5::REP_CMD_NOT_SUPPORTED).await?;
    Ok(())
}

/// A `CONNECT` request: one route decision, the reply and the transfer to the end.
async fn serve_connect(
    mut stream: TcpStream,
    cfg: ProxyConfig,
    request: socks5::Request,
    started: Instant,
) -> Result<()> {
    let target_text = request.target.to_string();
    let opening = tokio::select! {
        opened = path::open_chosen_route(&cfg, &request.target, request.port, "socks") => Some(opened),
        () = client_gone(&stream) => None,
    };
    let outcome = match opening {
        None => ProxyOutcome::abandoned(),
        Some(Ok(opened)) => {
            socks5::write_reply(&mut stream, socks5::REP_OK).await?;
            path::relay_bytes(opened, &cfg.route, stream, &[]).await
        }
        Some(Err(refused)) => {
            socks5::write_reply(&mut stream, socks5::REP_HOST_UNREACHABLE).await?;
            refused.outcome
        }
    };
    say(
        "socks",
        outcome_line(&target_text, &outcome, started.elapsed().as_millis()),
    );
    Ok(())
}

/// Completes when the client has closed the connection (`peek` gave `Ok(0)` or an error). If data arrived it
/// never completes: it waits for `relay_bytes` in the socket, such a client has not "left".
async fn client_gone(client: &TcpStream) {
    let mut byte = [0u8; 1];
    match client.peek(&mut byte).await {
        Ok(n) if n > 0 => std::future::pending::<()>().await,
        _ => {}
    }
}

#[cfg(test)]
#[path = "serve_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "serve_access_tests.rs"]
mod access_tests;
