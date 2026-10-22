//! The per-stream loop of the UDP gateway: one UDP socket per `conn_id`, replies go back as frames.

use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{sleep_until, Instant};

use super::codec::{encode, read_frame, Frame, Target, FLAG_CLOSE, FLAG_DATA, FLAG_KEEPALIVE};
use crate::exit::outbound::{is_forbidden, is_own_address, udp_address};
use crate::exit::server::ExitConfig;
use crate::node::output::{say, short_peer_id};
use crate::transport::net::{BoxRecv, BoxSend, PeerId};

/// A session without traffic in either direction lives this long.
const SESSION_IDLE: Duration = Duration::from_secs(60);
/// Sessions (`conn_id`s) per stream; new ones beyond it are dropped.
const MAX_SESSIONS: usize = 256;
/// Replies waiting for the stream writer.
const REPLY_QUEUE: usize = 64;

#[derive(Clone, Copy)]
pub(super) struct Limits {
    pub idle: Duration,
    pub max_sessions: usize,
}

const LIMITS: Limits = Limits {
    idle: SESSION_IDLE,
    max_sessions: MAX_SESSIONS,
};

struct Session {
    sock: Arc<UdpSocket>,
    last: Arc<Mutex<Instant>>,
    task: JoinHandle<()>,
}

impl Drop for Session {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Session {
    fn touch(&self) {
        *self.last.lock().expect("session clock") = Instant::now();
    }
}

struct Gateway {
    cfg: ExitConfig,
    peer: PeerId,
    limits: Limits,
    replies: mpsc::Sender<Vec<u8>>,
    sessions: HashMap<u16, Session>,
    /// Destinations that passed the address guard in this stream.
    allowed: HashSet<SocketAddr>,
    /// Connections already reported as refused (one log line per session).
    refused: HashSet<u16>,
}

/// Serves the gateway on the stream until it closes; all sockets close with it.
pub(in crate::exit) async fn serve(cfg: ExitConfig, peer: PeerId, send: BoxSend, recv: BoxRecv) {
    run(cfg, peer, send, recv, LIMITS).await;
}

pub(super) async fn run(
    cfg: ExitConfig,
    peer: PeerId,
    send: BoxSend,
    mut recv: BoxRecv,
    limits: Limits,
) {
    let (replies, queue) = mpsc::channel(REPLY_QUEUE);
    let writer = tokio::spawn(write_replies(send, queue));
    let mut gateway = Gateway {
        cfg,
        peer,
        limits,
        replies,
        sessions: HashMap::new(),
        allowed: HashSet::new(),
        refused: HashSet::new(),
    };
    while let Ok(frame) = read_frame(&mut recv).await {
        gateway.handle(frame).await;
    }
    writer.abort();
}

async fn write_replies<W: AsyncWrite + Unpin>(mut send: W, mut queue: mpsc::Receiver<Vec<u8>>) {
    while let Some(bytes) = queue.recv().await {
        if send.write_all(&bytes).await.is_err() {
            return;
        }
    }
}

impl Gateway {
    async fn handle(&mut self, frame: Frame) {
        if frame.flags & FLAG_KEEPALIVE != 0 {
            return self.reply(&Frame::keepalive(frame.conn_id)).await;
        }
        if frame.flags & FLAG_CLOSE != 0 {
            self.sessions.remove(&frame.conn_id);
            return;
        }
        if frame.flags & FLAG_DATA != 0 {
            return self.datagram(frame).await;
        }
        self.reply(&Frame::error(frame.conn_id)).await;
    }

    async fn reply(&self, frame: &Frame) {
        let Some(bytes) = encode(frame) else { return };
        let _ = self.replies.send(bytes).await;
    }

    async fn datagram(&mut self, frame: Frame) {
        let Some(target) = frame.target else { return };
        let Some(dest) = self.destination(frame.conn_id, &target).await else {
            return;
        };
        let Some(session) = self.session(frame.conn_id, dest) else {
            return;
        };
        session.touch();
        let _ = session.sock.send_to(&frame.data, dest).await;
    }

    /// The address to send to, or `None` if it is closed to the exit or does not resolve.
    async fn destination(&mut self, conn_id: u16, target: &Target) -> Option<SocketAddr> {
        let (addr, trusted) = self.resolve(target).await?;
        if trusted || self.allowed.contains(&addr) {
            return Some(addr);
        }
        if is_forbidden(addr.ip()) || is_own_address(addr.ip()) {
            self.report_refused(conn_id);
            return None;
        }
        self.allowed.insert(addr);
        Some(addr)
    }

    /// The address and whether it comes from the test table (it bypasses the address guard).
    async fn resolve(&self, target: &Target) -> Option<(SocketAddr, bool)> {
        match target {
            Target::Ip(addr) => Some((*addr, false)),
            Target::Domain(name, port) => self.resolve_name(name, *port).await,
        }
    }

    async fn resolve_name(&self, name: &str, port: u16) -> Option<(SocketAddr, bool)> {
        if let Some(addr) = self.cfg.host_overrides.get(name) {
            return Some((*addr, true));
        }
        if let Ok(ip) = name.parse::<IpAddr>() {
            return Some((SocketAddr::new(ip, port), false));
        }
        Some((udp_address(&self.cfg.resolver, name, port).await?, false))
    }

    fn report_refused(&mut self, conn_id: u16) {
        if self.refused.insert(conn_id) {
            say(
                "exit",
                format!(
                    "{}: udpgw {conn_id}: destination closed to exit",
                    short_peer_id(&self.peer)
                ),
            );
        }
    }

    /// The live session of `conn_id`, created on first use; `None` over the limit or if no socket opened.
    fn session(&mut self, conn_id: u16, dest: SocketAddr) -> Option<&Session> {
        self.sessions.retain(|_, s| !s.task.is_finished());
        if !self.sessions.contains_key(&conn_id) {
            if self.sessions.len() >= self.limits.max_sessions {
                return None;
            }
            let session = open_session(conn_id, dest, self.limits.idle, self.replies.clone())?;
            self.sessions.insert(conn_id, session);
        }
        self.sessions.get(&conn_id)
    }
}

fn open_session(
    conn_id: u16,
    dest: SocketAddr,
    idle: Duration,
    replies: mpsc::Sender<Vec<u8>>,
) -> Option<Session> {
    let local: SocketAddr = if dest.is_ipv4() {
        ([0, 0, 0, 0], 0).into()
    } else {
        ([0u16; 8], 0).into()
    };
    let std_sock = std::net::UdpSocket::bind(local).ok()?;
    std_sock.set_nonblocking(true).ok()?;
    let sock = Arc::new(UdpSocket::from_std(std_sock).ok()?);
    let last = Arc::new(Mutex::new(Instant::now()));
    let task = tokio::spawn(pump_replies(
        conn_id,
        sock.clone(),
        last.clone(),
        idle,
        replies,
    ));
    Some(Session { sock, last, task })
}

/// Turns datagrams arriving at the session socket into reply frames until the session is idle.
async fn pump_replies(
    conn_id: u16,
    sock: Arc<UdpSocket>,
    last: Arc<Mutex<Instant>>,
    idle: Duration,
    replies: mpsc::Sender<Vec<u8>>,
) {
    let mut buf = vec![0u8; 65_535];
    loop {
        let deadline = *last.lock().expect("session clock") + idle;
        tokio::select! {
            _ = sleep_until(deadline) => {
                if Instant::now() >= *last.lock().expect("session clock") + idle {
                    return;
                }
            }
            got = sock.recv_from(&mut buf) => {
                let Ok((n, from)) = got else { return };
                *last.lock().expect("session clock") = Instant::now();
                let Some(bytes) = encode(&Frame::data(conn_id, from, buf[..n].to_vec())) else { continue };
                if replies.send(bytes).await.is_err() {
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
