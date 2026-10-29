//! Handler of incoming exit protocol connections: permission, request frame, connection to
//! the domain and byte shuttling (`super::splice`).

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use crate::transport::net::{BoxFuture, BoxRecv, BoxSend, PeerConn, PeerId, ServiceHandler};
use anyhow::{anyhow, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::iface_bind::OutIface;
use crate::members::membership::Members;
use crate::node::output::{say, short_peer_id};
use crate::node::tcp_tune::disable_nagle;
use crate::proxy::domains::DomainList;
use crate::site_checks::doh::Resolver;

use super::clients::{Clients, ConnRef};
use super::frame::{
    parse_exit_request, EXIT_BUSY_REPLY, EXIT_CODE_BAD_REQUEST, EXIT_CODE_OK, EXIT_CODE_REFUSED,
    EXIT_PROTOCOL_VERSION,
};
use super::limits::{Busy, ExitLimits};
use super::outbound::{open_target, Refusal};
use super::report;
use super::splice::splice_windowed;
use super::stream_tasks::StreamTasks;
use super::traffic::{Direction, ExitTraffic};
use super::udpgw;

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
    /// Live exit streams of the process against the stream limit and the memory guard.
    pub limits: ExitLimits,
    /// The interface exit connections are bound to; `None` means the system routing table.
    pub iface: OutIface,
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
            crate::node::output::spawn(report::log_link_stats_while_open(
                conn.clone(),
                peer,
                "exit",
            ));
            cfg.traffic.watch(conn.clone(), Direction::ThroughUs);
            // The guard lives for the whole loop: once the connection closes or `handle` is dropped, the streams are forgotten at once.
            let conn_guard = cfg.clients.register_conn(peer);
            // Declared after the guard, so on exit the stream tasks are aborted first: a stream outliving its
            // connection would hold its slot and its socket to the target until the half-close grace ends.
            let mut tasks = StreamTasks::default();
            // The connection closed — that is the normal end of work, not an error.
            while let Ok(pair) = conn.accept().await {
                let cfg = cfg.clone();
                let conn_ref = conn_guard.conn();
                let task = crate::node::output::spawn(async move {
                    if let Err(e) =
                        serve_exit_stream(cfg, peer, conn_ref, pair.send, pair.recv).await
                    {
                        say(
                            "exit",
                            format!("{}: stream closed: {e:#}", short_peer_id(&peer)),
                        );
                    }
                });
                tasks.push(task.abort_handle());
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

/// Checks the permission and reads the request frame. `None` means the refusal or the
/// bad-request reply has already been written and the stream is done.
async fn read_exit_request(
    cfg: &ExitConfig,
    peer: PeerId,
    send: &mut BoxSend,
    recv: &mut BoxRecv,
) -> Result<Option<(String, u16)>> {
    // 1. Permission: the exit is on and the caller is a network member.
    if !cfg.enabled.load(Ordering::Relaxed) || !cfg.members.is_member(peer) {
        write_exit_reply(send, EXIT_CODE_REFUSED).await?;
        return Ok(None);
    }
    // 2. Request frame: first two bytes, then the name and port.
    let raw = match read_request_raw(recv).await {
        Ok(raw) => raw,
        Err(()) => {
            write_exit_reply(send, EXIT_CODE_BAD_REQUEST).await?;
            return Ok(None);
        }
    };
    match parse_exit_request(&raw) {
        Ok(v) => Ok(Some(v)),
        Err(e) => {
            say(
                "exit",
                format!(
                    "{}: request could not be parsed: {e:#}",
                    short_peer_id(&peer)
                ),
            );
            write_exit_reply(send, EXIT_CODE_BAD_REQUEST).await?;
            Ok(None)
        }
    }
}

/// The conventional UDP gateway target: served on this stream, no address lookup or connection.
async fn serve_gateway(
    cfg: ExitConfig,
    peer: PeerId,
    conn: ConnRef,
    mut send: BoxSend,
    recv: BoxRecv,
) -> Result<()> {
    write_exit_reply(&mut send, EXIT_CODE_OK).await?;
    let _stream_guard = conn.register_stream();
    say("exit", format!("{}: udpgw opened", short_peer_id(&peer)));
    udpgw::serve(cfg, peer, send, recv).await;
    Ok(())
}

/// Answers one request and, if everything matches, shuttles bytes.
async fn serve_exit_stream(
    cfg: ExitConfig,
    peer: PeerId,
    conn: ConnRef,
    mut send: BoxSend,
    mut recv: BoxRecv,
) -> Result<()> {
    let Some((target_host, port)) = read_exit_request(&cfg, peer, &mut send, &mut recv).await?
    else {
        return Ok(());
    };
    // The slot lives until the end of this future, including when the task is aborted.
    let _slot = match cfg.limits.try_admit() {
        Ok(slot) => slot,
        Err(busy) => return refuse_busy(&mut send, &peer, &cfg.limits, busy).await,
    };
    if udpgw::is_gateway_target(&target_host, port) {
        return serve_gateway(cfg, peer, conn, send, recv).await;
    }
    // 3. Address and connection: the test table, otherwise a literal or the own resolver over HTTPS (IPv4 first,
    // then IPv6); closed networks are refused (`address_guard`). Neither domain nor port is checked (those rules
    // were removed).
    let tcp = match open_target(&cfg, &target_host, port).await {
        Ok(tcp) => tcp,
        Err(refusal) => return refuse(&mut send, &peer, refusal).await,
    };
    disable_nagle(&tcp);
    write_exit_reply(&mut send, EXIT_CODE_OK).await?;
    // The client is registered only after `EXIT_CODE_OK`; the guard lives until the end of `splice` (including on `abort`).
    let _stream_guard = conn.register_stream();
    say(
        "exit",
        format!("{}: {target_host}:{port} opened", short_peer_id(&peer)),
    );
    // Byte counts are needed only by the requesting side, the exit doesn't print them.
    let _ = splice_windowed(send, recv, tcp, cfg.limits.stream_window_bytes()).await;
    Ok(())
}

/// One log line and the refusal code for the member.
async fn refuse(send: &mut BoxSend, peer: &PeerId, refusal: Refusal) -> Result<()> {
    say("exit", format!("{}: {}", short_peer_id(peer), refusal.line));
    write_exit_reply(send, refusal.code).await
}

/// The short busy reply: the version byte, then the write side is closed so the client reads the end of the
/// stream after it. The log line is rate-limited: under load refusals come by the hundreds per second.
async fn refuse_busy(
    send: &mut BoxSend,
    peer: &PeerId,
    limits: &ExitLimits,
    busy: Busy,
) -> Result<()> {
    if let Some(refused) = limits.busy_line_due(std::time::Instant::now()) {
        let held = match refused {
            0 => String::new(),
            n => format!(" (+{n} refused, not logged)"),
        };
        say(
            "exit",
            format!("{}: {}{held}", short_peer_id(peer), busy.line()),
        );
    }
    send.write_all(&EXIT_BUSY_REPLY)
        .await
        .map_err(|e| anyhow!("exit reply wasn't sent: {e}"))?;
    send.shutdown()
        .await
        .map_err(|e| anyhow!("exit reply wasn't finished: {e}"))
}

async fn write_exit_reply(send: &mut BoxSend, code: u8) -> Result<()> {
    send.write_all(&[EXIT_PROTOCOL_VERSION, code])
        .await
        .map_err(|e| anyhow!("exit reply wasn't sent: {e}"))?;
    Ok(())
}

#[cfg(test)]
#[path = "server_frame_tests.rs"]
mod frame_tests;
#[cfg(test)]
#[path = "server_limits_tests.rs"]
mod limits_tests;
#[cfg(test)]
#[path = "server_iface_tests.rs"]
mod server_iface_tests;
#[cfg(test)]
#[path = "server_service_fixture.rs"]
mod service_fixture;
#[cfg(test)]
#[path = "server_service_tests.rs"]
mod service_tests;
#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "server_udpgw_tests.rs"]
mod udpgw_tests;
