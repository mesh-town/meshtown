//! Session setup: the table entry, the header and `Ack` exchange, and the start of the control stream loops.

use super::ctrl;
use super::hs::{expect_ack, send_header};
use super::{HyperConn, HyperSessionState, StreamDialer, STREAM_QUEUE};
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::id::PeerId;
use crate::transport::net_hyper::link::LinkMeter;
use crate::transport::net_hyper::table::Sessions;
use crate::transport::net_hyper::wire::{write_reply, Ctrl, Header, Reply};
use std::sync::{Arc, OnceLock};
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, watch};
use townjump_dht::Duplex;

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
