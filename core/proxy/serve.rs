//! The proxy's listening socket (SOCKS5 and HTTP on one port) and the SOCKS5 "face": parsing the request and
//! replying to the client; the route to the target is in `path`, HTTP is in `http`.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use tokio::net::{TcpListener, TcpStream};

use crate::config::config_file::ListenAccess;
use crate::exit::PeerConnPool;
use crate::node::output::say;
use crate::node::tcp_tune::disable_nagle;
use crate::proxy::route::RouteCtx;
use crate::proxy::socks5;
use crate::site_checks::doh::Resolver;
use crate::transport::net::NetStack;
use crate::web_gui::auth::access::bind_ip;
use crate::web_gui::auth::iface_os::current_view;

use super::admit::{admit_conn, iface_deny_text, is_proxy_client_allowed, Admission, DenyLog};
use super::front::NetDirectory;
use super::outcome::{outcome_line, ProxyOutcome};
use super::{assoc, http, mesh, path};

/// The default port of the local SOCKS5 server.
pub const DEFAULT_SOCKS_PORT: u16 = 1080;
pub const DIRECT_TCP_CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
/// The reason of a refusal while the proxy has no network to go through.
pub const NO_PROXY_NETWORK: &str = "no network for the proxy";
/// How long a client has, from `accept`, to deliver the whole request head (the first byte included).
/// It is a deadline, not an idle limit: a client dripping bytes cannot hold the task. A connection that
/// misses it is closed silently.
const HEAD_DEADLINE: Duration = Duration::from_secs(30);

/// Everything the SOCKS5 server needs to work.
#[derive(Clone)]
pub struct ProxyConfig {
    pub route: RouteCtx,
    pub stack: NetStack,
    pub pool: PeerConnPool,
    pub resolver: Arc<Resolver>,
    /// Domain addresses that bypass DNS: always empty in the node, filled only by tests.
    pub host_overrides: Arc<HashMap<String, SocketAddr>>,
    /// Free places for UDP associations on this node (`udp_slots()`).
    pub udp_slots: Arc<tokio::sync::Semaphore>,
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
            crate::cli::program_name::program_name()
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
/// is in `serve_front`). A busy port is an error, the node does not start.
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
/// The target is read at the moment of `accept` and lives with the connection; with none, requests are refused.
pub(crate) async fn serve_front(
    listener: TcpListener,
    target: tokio::sync::watch::Receiver<Option<ProxyConfig>>,
    nets: NetDirectory,
    adm: Admission,
) {
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
        let cfg = target.borrow().clone();
        let deadline = tokio::time::Instant::now() + HEAD_DEADLINE;
        crate::node::output::spawn(dispatch(stream, cfg, nets.clone(), client_addr, deadline));
    }
}

/// Tells the protocol apart by the first byte without taking it from the stream: `0x05` is SOCKS5,
/// otherwise HTTP; a connection closed without a single byte is a quiet exit. It prints the error with the
/// protocol label. A client that sends no byte before `deadline` is closed silently.
async fn dispatch(
    stream: TcpStream,
    cfg: Option<ProxyConfig>,
    nets: NetDirectory,
    client_addr: SocketAddr,
    deadline: tokio::time::Instant,
) {
    let mut first = [0u8; 1];
    match tokio::time::timeout_at(deadline, stream.peek(&mut first)).await {
        Ok(Ok(n)) if n > 0 => {}
        _ => return,
    }
    if first[0] == socks5::VERSION {
        if let Err(e) = serve_socks5_connection(stream, cfg, nets, deadline).await {
            say("socks", format!("{client_addr}: {e:#}"));
        }
        return;
    }
    if let Err(e) = http::serve_http_connection(stream, cfg, nets, deadline).await {
        say("http", format!("{client_addr}: {e:#}"));
    }
}

/// Serves one SOCKS5 connection from the greeting to the end of the transfer. There is one route decision for
/// the whole connection and it does not depend on the target (a domain or an address);
/// a connection through an exit is closed if the exit is removed from `route.exits` while it is still open.
/// The greeting and the request bytes must arrive before `deadline`, otherwise the connection is closed silently.
async fn serve_socks5_connection(
    mut stream: TcpStream,
    cfg: Option<ProxyConfig>,
    nets: NetDirectory,
    deadline: tokio::time::Instant,
) -> Result<()> {
    // One timestamp for the whole serving: the summary line prints it.
    let started = Instant::now();
    let head = tokio::time::timeout_at(deadline, read_request_head(&mut stream)).await;
    let Ok(raw) = head else {
        return Ok(());
    };
    let raw = raw?;
    let request = match socks5::parse_socks5_request(&raw) {
        Ok(request) => request,
        Err(e) => {
            // The request was not parsed — there is no target, nothing to write a summary line about.
            say("socks", format!("request rejected: {e}"));
            socks5::write_reply(&mut stream, e.reply_code()).await?;
            return Ok(());
        }
    };
    let Some(cfg) = cfg else {
        let target = match request.command {
            socks5::Command::Connect => request.target.to_string(),
            socks5::Command::UdpAssociate => "udp associate".to_string(),
        };
        return refuse_no_network(stream, &target, started).await;
    };
    match request.command {
        socks5::Command::Connect => serve_connect(stream, cfg, nets, request, started).await,
        socks5::Command::UdpAssociate => assoc::serve_associate(stream, cfg, started).await,
    }
}

/// The greeting (with its reply) and the raw bytes of the request.
async fn read_request_head(stream: &mut TcpStream) -> Result<Vec<u8>> {
    socks5::read_greeting(stream).await?;
    socks5::read_request_bytes(stream).await
}

/// The reply and the journal line for a request that arrived while the proxy has no network.
async fn refuse_no_network(mut stream: TcpStream, target: &str, started: Instant) -> Result<()> {
    socks5::write_reply(&mut stream, socks5::REP_HOST_UNREACHABLE).await?;
    let outcome = ProxyOutcome::refused("no route", NO_PROXY_NETWORK);
    say(
        "socks",
        outcome_line(target, &outcome, started.elapsed().as_millis()),
    );
    Ok(())
}

/// A `CONNECT` request: one route decision, the reply and the transfer to the end.
async fn serve_connect(
    mut stream: TcpStream,
    cfg: ProxyConfig,
    nets: NetDirectory,
    request: socks5::Request,
    started: Instant,
) -> Result<()> {
    let target_text = request.target.to_string();
    let opening = tokio::select! {
        opened = mesh::open::open_target(&cfg, &nets, &request.target, request.port, "socks") => Some(opened),
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
pub(in crate::proxy) async fn client_gone(client: &TcpStream) {
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

#[cfg(test)]
#[path = "serve_head_tests.rs"]
mod head_tests;
