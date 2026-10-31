//! The per-stream loop of the UDP gateway: one UDP socket per `conn_id`, replies go back as frames.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{sleep_until, Instant};

use super::codec::{encode, read_frame, Frame, FLAG_CLOSE, FLAG_DATA, FLAG_KEEPALIVE};
use crate::exit::server::ExitConfig;
use crate::iface_bind::{bind_udp, check};
use crate::node::output::{say, short_peer_id};
use crate::transport::net::{BoxRecv, BoxSend, PeerId};

/// A session without traffic in either direction lives this long.
const SESSION_IDLE: Duration = Duration::from_secs(60);
/// Sessions (`conn_id`s) per stream; new ones beyond it are dropped.
const MAX_SESSIONS: usize = 256;
/// Replies waiting for the stream writer.
const REPLY_QUEUE: usize = 64;
/// Frames read off the stream and not yet handled.
const FRAME_QUEUE: usize = 64;

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
    /// The name the last datagram of a lookup went to, so that its next frames skip the lookup.
    named: Option<Named>,
    /// The interface the socket was opened with; a different current value reopens the session.
    iface: Option<Arc<str>>,
}

struct Named {
    name: String,
    port: u16,
    addr: SocketAddr,
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
    /// `conn_id`s whose name is being looked up, with the frames waiting behind it.
    pending: HashMap<u16, Pending>,
    /// Ends of lookups come back here.
    results: mpsc::Sender<Resolved>,
    next_lookup: u64,
}

/// Serves the gateway on the stream until it closes; all sockets close with it.
pub(in crate::exit) async fn serve(cfg: ExitConfig, peer: PeerId, send: BoxSend, recv: BoxRecv) {
    run(cfg, peer, send, recv, LIMITS).await;
}

pub(super) async fn run(
    cfg: ExitConfig,
    peer: PeerId,
    send: BoxSend,
    recv: BoxRecv,
    limits: Limits,
) {
    let (replies, queue) = mpsc::channel(REPLY_QUEUE);
    let (results, mut ended) = mpsc::channel(MAX_RESOLVING);
    let (sender, mut frames) = mpsc::channel(FRAME_QUEUE);
    let reader = crate::node::output::spawn(read_frames(recv, sender));
    let writer = crate::node::output::spawn(write_replies(send, queue));
    let mut gateway = Gateway {
        cfg,
        peer,
        limits,
        replies,
        sessions: HashMap::new(),
        allowed: HashSet::new(),
        refused: HashSet::new(),
        pending: HashMap::new(),
        results,
        next_lookup: 0,
    };
    loop {
        tokio::select! {
            frame = frames.recv() => match frame {
                Some(frame) => gateway.handle(frame).await,
                None => break,
            },
            Some(done) = ended.recv() => gateway.resolved(done).await,
        }
    }
    reader.abort();
    writer.abort();
}

/// Reads frames off the stream until it closes or breaks; the loop above never waits on a half-read frame.
async fn read_frames(mut recv: BoxRecv, frames: mpsc::Sender<Frame>) {
    while let Ok(frame) = read_frame(&mut recv).await {
        if frames.send(frame).await.is_err() {
            return;
        }
    }
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
            self.pending.remove(&frame.conn_id);
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

    /// The live session of `conn_id`, created on first use; `None` over the limit, if the exit's interface
    /// cannot be used, or if no socket opened. A session opened with another interface than the current
    /// one is closed and opened again.
    fn session(&mut self, conn_id: u16, dest: SocketAddr) -> Option<&Session> {
        self.sessions.retain(|_, s| !s.task.is_finished());
        let iface = self.cfg.iface.current();
        if self
            .sessions
            .get(&conn_id)
            .is_some_and(|s| s.iface != iface)
        {
            self.sessions.remove(&conn_id);
        }
        if !self.sessions.contains_key(&conn_id) {
            if self.sessions.len() >= self.limits.max_sessions {
                return None;
            }
            if !self.iface_usable(conn_id, iface.as_deref()) {
                return None;
            }
            let replies = self.replies.clone();
            let session = open_session(conn_id, dest, self.limits.idle, replies, iface.as_deref())?;
            self.sessions.insert(conn_id, session);
        }
        self.sessions.get(&conn_id)
    }

    /// Whether sockets can be bound to the interface; a refusal is logged once per `conn_id`.
    fn iface_usable(&mut self, conn_id: u16, iface: Option<&str>) -> bool {
        let Some(name) = iface else { return true };
        let Err(e) = check(name) else { return true };
        if self.refused.insert(conn_id) {
            say(
                "exit",
                format!(
                    "{}: udpgw {conn_id}: exit {e}; not sent through other networks",
                    short_peer_id(&self.peer)
                ),
            );
        }
        false
    }
}

fn open_session(
    conn_id: u16,
    dest: SocketAddr,
    idle: Duration,
    replies: mpsc::Sender<Vec<u8>>,
    iface: Option<&str>,
) -> Option<Session> {
    let std_sock = bind_udp(dest, iface).ok()?;
    std_sock.set_nonblocking(true).ok()?;
    let sock = Arc::new(UdpSocket::from_std(std_sock).ok()?);
    let last = Arc::new(Mutex::new(Instant::now()));
    let task = crate::node::output::spawn(pump_replies(
        conn_id,
        sock.clone(),
        last.clone(),
        idle,
        replies,
    ));
    let iface = iface.map(Arc::from);
    Some(Session {
        sock,
        last,
        task,
        named: None,
        iface,
    })
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

#[path = "session_pending.rs"]
mod pending;
use pending::{Pending, Resolved, MAX_RESOLVING};

#[cfg(test)]
#[path = "iface_tests.rs"]
mod iface_tests;
#[cfg(test)]
#[path = "session_resolve_tests.rs"]
mod resolve_tests;
#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
