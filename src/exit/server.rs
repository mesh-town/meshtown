//! Handler of incoming exit protocol connections: permission, request frame, connection to
//! the domain and byte shuttling (`super::splice`).

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use crate::net::{BoxFuture, BoxRecv, BoxSend, PeerConn, PeerId, ServiceHandler};
use anyhow::{anyhow, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::doh::Resolver;
use crate::domains::DomainList;
use crate::membership::Members;
use crate::output::{say, short_peer_id};
use crate::tcp_tune::disable_nagle;

use super::address_guard::{first_allowed, is_forbidden, is_own_address};
use super::clients::{Clients, ConnRef};
use super::frame::{
    parse_exit_request, EXIT_CODE_ADDRESS_FORBIDDEN, EXIT_CODE_BAD_REQUEST, EXIT_CODE_CONNECT_FAILED, EXIT_CODE_NO_ADDRESS,
    EXIT_CODE_OK, EXIT_CODE_REFUSED, EXIT_TCP_CONNECT_TIMEOUT, EXIT_PROTOCOL_VERSION,
};
use super::report;
use super::udpgw;
use super::splice::splice;
use super::traffic::{Direction, ExitTraffic};

/// Exit service settings.
#[derive(Clone)]
pub struct ExitConfig {
    /// Whether the node acts as an exit; changed by the console command `exit on` / `exit off`.
    pub enabled: Arc<AtomicBool>,
    /// Whoever passed the handshake has the right to ask.
    pub members: Members,
    /// The node's own domain list. Filled in `node::start_node` right after the list is opened; the rule
    /// refusing a domain not on the list has been removed — the exit owner serves any domain and controls
    /// only `exit.enabled`, the field stays in reserve for a future exit switch of its own.
    pub list: Arc<OnceLock<DomainList>>,
    /// The node's resolver.
    pub resolver: Arc<Resolver>,
    /// Domain addresses bypassing DNS: always empty in a node, filled only by tests.
    /// The value replaces both address and port; it bypasses the address filter
    /// and the own-address probe (`address_guard`).
    pub host_overrides: Arc<HashMap<String, SocketAddr>>,
    /// Registry of exit clients: `serve_exit_stream` adds a stream after `EXIT_CODE_OK`, the snapshot reads the list.
    pub clients: Clients,
    /// Registry of exit traffic speed: incoming connections are counted as `ThroughUs`.
    pub traffic: ExitTraffic,
}

/// Handler of incoming exit service connections.
#[derive(Clone)]
struct ExitService {
    cfg: ExitConfig,
}

/// The exit handler for `NetStack`.
pub fn exit_service(cfg: ExitConfig) -> Arc<dyn ServiceHandler> {
    Arc::new(ExitService { cfg })
}

impl ServiceHandler for ExitService {
    fn handle(&self, conn: Arc<dyn PeerConn>) -> BoxFuture<'static, ()> {
        let cfg = self.cfg.clone();
        Box::pin(async move {
            let peer = conn.peer();
            tokio::spawn(report::log_link_stats_while_open(conn.clone(), peer, "exit"));
            cfg.traffic.watch(conn.clone(), Direction::ThroughUs);
            // The guard lives for the whole loop: once the connection closes or `handle` is dropped, the streams are forgotten at once.
            let conn_guard = cfg.clients.register_conn(peer);
            // The connection closed — that is the normal end of work, not an error.
            while let Ok(pair) = conn.accept().await {
                let cfg = cfg.clone();
                let conn_ref = conn_guard.conn();
                tokio::spawn(async move {
                    if let Err(e) = serve_exit_stream(cfg, peer, conn_ref, pair.send, pair.recv).await {
                        say("exit", format!("{}: stream closed: {e:#}", short_peer_id(&peer)));
                    }
                });
            }
        })
    }
}

/// Reads the raw bytes of the request frame: first two bytes, then the name and port by their length.
/// A read error is not distinguished further: the caller replies `EXIT_CODE_BAD_REQUEST` without details
/// (as before it was moved into a separate function).
async fn read_request_raw(recv: &mut BoxRecv) -> std::result::Result<Vec<u8>, ()> {
    let mut head = [0u8; 2];
    recv.read_exact(&mut head).await.map_err(|_| ())?;
    let mut tail = vec![0u8; usize::from(head[1]) + 2];
    recv.read_exact(&mut tail).await.map_err(|_| ())?;
    let mut raw = head.to_vec();
    raw.extend_from_slice(&tail);
    Ok(raw)
}

/// Destination address: the test table as is; an IP literal goes through the filter (the client
/// sends the address in the request frame as a name); otherwise the resolver cache, on a miss
/// DoH, and the same filter on every address of the answer. The chosen address must not be the node's own.
async fn resolve_address(
    cfg: &ExitConfig,
    target_host: &str,
    port: u16,
) -> std::result::Result<SocketAddr, Refusal> {
    if let Some(addr) = cfg.host_overrides.get(target_host) {
        return Ok(*addr);
    }
    if let Ok(ip) = target_host.parse::<IpAddr>() {
        return literal_address(ip, target_host, port).and_then(|addr| reject_own_address(addr, target_host));
    }
    let addrs = cfg
        .resolver
        .resolve_cached(target_host)
        .await
        .map_err(|e| Refusal::no_address(format!("{target_host} — DNS: {e:#}")))?;
    first_allowed_resolved_address(&addrs, target_host, port).and_then(|addr| reject_own_address(addr, target_host))
}

/// Exit refusal: the reply code for the member and the tail of the log line (`refuse` adds the short id).
#[derive(Debug)]
struct Refusal {
    code: u8,
    line: String,
}

impl Refusal {
    fn no_address(line: String) -> Refusal {
        Refusal {
            code: EXIT_CODE_NO_ADDRESS,
            line,
        }
    }

    /// There is one reason and no address: an address from DoH is given out neither to the member nor to the log.
    fn forbidden(target_host: &str) -> Refusal {
        Refusal {
            code: EXIT_CODE_ADDRESS_FORBIDDEN,
            line: format!("{target_host} — address closed to exit"),
        }
    }
}

/// An IP literal from the frame: a closed one is refused; the exit doesn't open IPv6 (DoH asks only for A).
fn literal_address(
    ip: IpAddr,
    target_host: &str,
    port: u16,
) -> std::result::Result<SocketAddr, Refusal> {
    if is_forbidden(ip) {
        return Err(Refusal::forbidden(target_host));
    }
    if ip.is_ipv6() {
        return Err(Refusal::no_address(format!(
            "{target_host} — exit doesn't open IPv6"
        )));
    }
    Ok(SocketAddr::new(ip, port))
}

/// Addresses of a name from DoH: the first open one; an empty answer means "no address", all closed means "closed".
fn first_allowed_resolved_address(
    addrs: &[Ipv4Addr],
    target_host: &str,
    port: u16,
) -> std::result::Result<SocketAddr, Refusal> {
    if addrs.is_empty() {
        return Err(Refusal::no_address(format!(
            "{target_host} — no usable address"
        )));
    }
    first_allowed(addrs)
        .map(|ip| SocketAddr::from((ip, port)))
        .ok_or_else(|| Refusal::forbidden(target_host))
}

/// The node's own address — the same owner services as the loopback. Checked before connecting: a refusal
/// doesn't reveal whether the port is listening.
fn reject_own_address(addr: SocketAddr, target_host: &str) -> std::result::Result<SocketAddr, Refusal> {
    if is_own_address(addr.ip()) {
        return Err(Refusal::forbidden(target_host));
    }
    Ok(addr)
}

/// Answers one request and, if everything matches, shuttles bytes.
async fn serve_exit_stream(
    cfg: ExitConfig,
    peer: PeerId,
    conn: ConnRef,
    mut send: BoxSend,
    mut recv: BoxRecv,
) -> Result<()> {
    // 1. Permission: the exit is on and the caller is a network member.
    if !cfg.enabled.load(Ordering::Relaxed) || !cfg.members.is_member(peer) {
        return write_exit_reply(&mut send, EXIT_CODE_REFUSED).await;
    }
    // 2. Request frame: first two bytes, then the name and port.
    let raw = match read_request_raw(&mut recv).await {
        Ok(raw) => raw,
        Err(()) => return write_exit_reply(&mut send, EXIT_CODE_BAD_REQUEST).await,
    };
    let (target_host, port) = match parse_exit_request(&raw) {
        Ok(v) => v,
        Err(e) => {
            say(
                "exit",
                format!("{}: request could not be parsed: {e:#}", short_peer_id(&peer)),
            );
            return write_exit_reply(&mut send, EXIT_CODE_BAD_REQUEST).await;
        }
    };
    // The conventional UDP gateway target: served on this stream, no address lookup or connection.
    if udpgw::is_gateway_target(&target_host, port) {
        write_exit_reply(&mut send, EXIT_CODE_OK).await?;
        let _stream_guard = conn.register_stream();
        say("exit", format!("{}: udpgw opened", short_peer_id(&peer)));
        udpgw::serve(cfg, peer, send, recv).await;
        return Ok(());
    }
    // 3. Address: the test table, otherwise a literal or the own resolver over HTTPS; closed networks are
    // refused (`address_guard`). Neither domain nor port is checked (those rules were removed).
    let addr = match resolve_address(&cfg, &target_host, port).await {
        Ok(addr) => addr,
        Err(refusal) => return refuse(&mut send, &peer, refusal).await,
    };
    // 4. Connection to the address.
    let tcp = match open_tcp(&cfg, &target_host, addr).await {
        Ok(tcp) => tcp,
        Err(refusal) => return refuse(&mut send, &peer, refusal).await,
    };
    disable_nagle(&tcp);
    write_exit_reply(&mut send, EXIT_CODE_OK).await?;
    // The client is registered only after `EXIT_CODE_OK`; the guard lives until the end of `splice` (including on `abort`).
    let _stream_guard = conn.register_stream();
    say("exit", format!("{}: {target_host}:{port} opened", short_peer_id(&peer)));
    // Byte counts are needed only by the requesting side, the exit doesn't print them.
    let _ = splice(send, recv, tcp).await;
    Ok(())
}

/// Connection to an address. If it didn't open, the address may be stale in the cache — drop it.
async fn open_tcp(
    cfg: &ExitConfig,
    target_host: &str,
    addr: SocketAddr,
) -> std::result::Result<TcpStream, Refusal> {
    let Ok(Ok(tcp)) = timeout(EXIT_TCP_CONNECT_TIMEOUT, TcpStream::connect(addr)).await else {
        cfg.resolver.forget(target_host);
        return Err(Refusal {
            code: EXIT_CODE_CONNECT_FAILED,
            line: format!("{target_host} ({addr}) — connection didn't open"),
        });
    };
    Ok(tcp)
}

/// One log line and the refusal code for the member.
async fn refuse(send: &mut BoxSend, peer: &PeerId, refusal: Refusal) -> Result<()> {
    say("exit", format!("{}: {}", short_peer_id(peer), refusal.line));
    write_exit_reply(send, refusal.code).await
}

async fn write_exit_reply(send: &mut BoxSend, code: u8) -> Result<()> {
    send.write_all(&[EXIT_PROTOCOL_VERSION, code])
        .await
        .map_err(|e| anyhow!("exit reply wasn't sent: {e}"))?;
    Ok(())
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "server_service_tests.rs"]
mod service_tests;
#[cfg(test)]
#[path = "server_udpgw_tests.rs"]
mod udpgw_tests;
