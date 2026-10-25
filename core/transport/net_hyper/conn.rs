//! A `hyper` session: a control stream plus data streams, each one a separate DHT connection.
//! `establish_dialed_session` and `establish_accepted_session` run the header and `Ack` exchange, `HyperConn` implements `PeerConn`.
//! The table entry is made before `Open` on the dialing side and before `Ack` on the accepting side: a stream that arrives right
//! after the reply must find the session.

mod close_log;
mod ctrl;
mod hs;

use hs::{expect_ack, send_header};

use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::id::PeerId;
use crate::transport::net::link::LinkInfo;
use crate::transport::net::stream::{BoxFuture, StreamPair};
use crate::transport::net::transport::PeerConn;
use crate::transport::net_hyper::link::{link_info, path_text_of, LinkMeter, Metered, PathFacts};
use crate::transport::net_hyper::table::{SessionKey, Sessions};
use crate::transport::net_hyper::wire::Ctrl;
use crate::transport::net_hyper::wire::REASON_MAX_BYTES;
use crate::transport::net_hyper::wire::{write_reply, Header, Owner, Reply};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, watch};
use townjump_dht::Duplex;

/// How often the control stream sends `Ping`.
pub const PING_EVERY: Duration = Duration::from_secs(5);
/// How long the control stream may stay silent before the session is considered dead.
pub const DEAD_AFTER: Duration = Duration::from_secs(20);
/// Close code used when the other side went silent.
pub const CODE_SILENT: u32 = 0xFFFF_0002;
/// How many accepted streams not yet picked up by `accept()` the session holds.
const STREAM_QUEUE: usize = 256;

/// Dials a data stream to the other side: a DHT connection whose key has already been checked against `to`.
pub trait StreamDialer: Send + Sync {
    fn dial(&self, to: PeerId) -> BoxFuture<'_, NetResult<Box<dyn Duplex>>>;
}

/// Shared session state: held by the control stream loops and the handle.
pub(crate) struct HyperSessionState {
    peer: PeerId,
    session: u64,
    numbered_by_us: bool,
    dialer: Arc<dyn StreamDialer>,
    table: Arc<Sessions>,
    meter: Arc<LinkMeter>,
    out: mpsc::UnboundedSender<Ctrl>,
    close: OnceLock<(u32, Vec<u8>)>,
    closed_tx: watch::Sender<bool>,
}

pub(super) async fn wait_until_closed(rx: &mut watch::Receiver<bool>) {
    let _ = rx.wait_for(|c| *c).await;
}

impl HyperSessionState {
    fn key(&self) -> SessionKey {
        SessionKey {
            peer: self.peer,
            session: self.session,
            numbered_by_us: self.numbered_by_us,
        }
    }

    fn subscribe(&self) -> watch::Receiver<bool> {
        self.closed_tx.subscribe()
    }

    fn send(&self, frame: Ctrl) -> bool {
        self.out.send(frame).is_ok()
    }

    fn announce(&self) {
        self.meter.close();
        self.table.remove(self.key());
        self.closed_tx.send_replace(true);
    }

    /// Closes the session from our side and sends `Close`; always `false` ("the loop ends").
    pub(super) fn close_local(&self, code: u32, reason: &[u8]) -> bool {
        let reason = reason[..reason.len().min(REASON_MAX_BYTES)].to_vec();
        if self.close.set((code, reason.clone())).is_ok() {
            close_log::say_close(&self.peer, self.session, close_log::Side::Us, code, &reason);
            self.send(Ctrl::Close { code, reason });
            self.announce();
        }
        false
    }

    /// Closes the session on a frame or on the other side dropping; always `false`.
    pub(super) fn close_remote(&self, code: u32, reason: &[u8]) -> bool {
        if self.close.set((code, reason.to_vec())).is_ok() {
            close_log::say_close(
                &self.peer,
                self.session,
                close_log::Side::Peer,
                code,
                reason,
            );
            self.announce();
        }
        false
    }

    fn is_closed(&self) -> bool {
        self.close.get().is_some()
    }

    fn owner(&self) -> Owner {
        match self.numbered_by_us {
            true => Owner::Dialer,
            false => Owner::Acceptor,
        }
    }

    async fn open_stream(&self) -> NetResult<StreamPair> {
        let mut stream = self.dialer.dial(self.peer).await?;
        send_header(
            &mut stream,
            &Header::Stream {
                session: self.session,
                owner: self.owner(),
            },
        )
        .await?;
        expect_ack(&mut stream).await?;
        Ok(self.metered(stream))
    }

    fn metered(&self, stream: Box<dyn Duplex>) -> StreamPair {
        let (r, w) = tokio::io::split(stream);
        self.meter_pair(StreamPair {
            send: Box::new(w),
            recv: Box::new(r),
        })
    }

    /// Wraps both halves of the stream with the session counter (for accepted streams too).
    fn meter_pair(&self, pair: StreamPair) -> StreamPair {
        StreamPair {
            send: Box::new(Metered::new(pair.send, self.meter.clone())),
            recv: Box::new(Metered::new(pair.recv, self.meter.clone())),
        }
    }
}

/// Session handle; the last dropped handle closes it with code 0.
pub struct HyperConn {
    core: Arc<HyperSessionState>,
    accept_rx: tokio::sync::Mutex<mpsc::Receiver<StreamPair>>,
}

impl Drop for HyperConn {
    fn drop(&mut self) {
        self.core.close_local(0, b"");
    }
}

/// Creates the session and the table entry; the control stream is not started yet.
fn register(
    peer: PeerId,
    session: u64,
    numbered_by_us: bool,
    dialer: Arc<dyn StreamDialer>,
    table: Arc<Sessions>,
) -> (Arc<HyperConn>, mpsc::UnboundedReceiver<Ctrl>) {
    let (out, out_rx) = mpsc::unbounded_channel();
    let (tx, rx) = mpsc::channel(STREAM_QUEUE);
    let core = Arc::new(HyperSessionState {
        peer,
        session,
        numbered_by_us,
        dialer,
        table,
        out,
        meter: Arc::new(LinkMeter::new()),
        close: OnceLock::new(),
        closed_tx: watch::channel(false).0,
    });
    core.table.insert(core.key(), tx);
    (
        Arc::new(HyperConn {
            core,
            accept_rx: tokio::sync::Mutex::new(rx),
        }),
        out_rx,
    )
}

/// Result of the exchange: `Ack` received (sent) — the loops are running, otherwise the session is forgotten.
fn start_loops_or_forget_session(
    conn: Arc<HyperConn>,
    ctrl: Box<dyn Duplex>,
    out: mpsc::UnboundedReceiver<Ctrl>,
    res: NetResult<()>,
) -> NetResult<Arc<HyperConn>> {
    match res {
        Ok(()) => {
            conn.core.meter.mark_established();
            ctrl::spawn(conn.core.clone(), ctrl, out);
            Ok(conn)
        }
        Err(e) => {
            conn.core.close_remote(0, b"");
            Err(e)
        }
    }
}

/// Dialing side: `Open` over the already open control stream, waits for `Ack`.
pub async fn establish_dialed_session(
    peer: PeerId,
    session: u64,
    service_name: &[u8],
    mut ctrl: Box<dyn Duplex>,
    dialer: Arc<dyn StreamDialer>,
    table: Arc<Sessions>,
) -> NetResult<Arc<HyperConn>> {
    let (conn, out) = register(peer, session, true, dialer, table);
    let open = Header::Open {
        session,
        service_name: service_name.to_vec(),
    };
    let res = match send_header(&mut ctrl, &open).await {
        Ok(()) => expect_ack(&mut ctrl).await,
        Err(e) => Err(e),
    };
    start_loops_or_forget_session(conn, ctrl, out, res)
}

/// Accepting side: the `Open` header has already been read and checked, sends `Ack`.
pub async fn establish_accepted_session(
    peer: PeerId,
    session: u64,
    mut ctrl: Box<dyn Duplex>,
    dialer: Arc<dyn StreamDialer>,
    table: Arc<Sessions>,
) -> NetResult<Arc<HyperConn>> {
    let (conn, out) = register(peer, session, false, dialer, table);
    let res = write_ack(&mut ctrl).await;
    start_loops_or_forget_session(conn, ctrl, out, res)
}

async fn write_ack(ctrl: &mut Box<dyn Duplex>) -> NetResult<()> {
    write_reply(ctrl, &Reply::Ack).await.map_err(NetError::Io)?;
    ctrl.flush().await.map_err(NetError::Io)
}

impl HyperConn {
    /// Records the path facts of the session (the first call wins).
    pub(crate) fn note_path(&self, facts: PathFacts) {
        self.core.meter.note_path(facts);
    }

    async fn open_inner(&self) -> NetResult<StreamPair> {
        let mut closed = self.core.subscribe();
        tokio::select! {
            biased;
            _ = wait_until_closed(&mut closed) => Err(NetError::Closed),
            res = self.core.open_stream() => res,
        }
    }

    async fn accept_inner(&self) -> NetResult<StreamPair> {
        let mut closed = self.core.subscribe();
        let mut rx = self.accept_rx.lock().await;
        tokio::select! {
            biased;
            _ = wait_until_closed(&mut closed) => Err(NetError::Closed),
            pair = rx.recv() => pair.map(|p| self.core.meter_pair(p)).ok_or(NetError::Closed),
        }
    }
}

impl PeerConn for HyperConn {
    fn peer(&self) -> PeerId {
        self.core.peer
    }

    fn stable_id(&self) -> u64 {
        self.core.session
    }

    fn open(&self) -> BoxFuture<'_, NetResult<StreamPair>> {
        Box::pin(self.open_inner())
    }

    fn accept(&self) -> BoxFuture<'_, NetResult<StreamPair>> {
        Box::pin(self.accept_inner())
    }

    fn link(&self) -> LinkInfo {
        link_info(&self.core.meter)
    }

    fn close(&self, code: u32, reason: &[u8]) {
        self.core.close_local(code, reason);
    }

    fn closed(&self) -> BoxFuture<'_, ()> {
        let mut rx = self.core.subscribe();
        Box::pin(async move { wait_until_closed(&mut rx).await })
    }

    fn is_closed(&self) -> bool {
        self.core.is_closed()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn path_text(&self) -> String {
        path_text_of(link_info(&self.core.meter).rtt)
    }

    fn path_lines(&self) -> Vec<String> {
        vec![self.path_text()]
    }

    fn close_reason(&self) -> String {
        let text = |(code, reason): &(u32, Vec<u8>)| {
            format!("code {code}: {}", String::from_utf8_lossy(reason))
        };
        self.core.close.get().map(text).unwrap_or_default()
    }
}

#[cfg(test)]
#[path = "conn_tests.rs"]
mod tests;
