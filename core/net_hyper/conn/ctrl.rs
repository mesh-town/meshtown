//! Control stream of a session: two loops (reading frames with a silence check and writing with pings).
//! The loops hold the shared state `HyperSessionState`, not the `HyperConn` handle, otherwise the last handle would never die.

use super::{wait_until_closed, HyperSessionState, CODE_SILENT, DEAD_AFTER, PING_EVERY};
use crate::net_hyper::wire::{read_ctrl, write_ctrl, Ctrl};
use townjump_dht::Duplex;
use std::io;
use std::sync::Arc;
use tokio::io::{AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::sync::mpsc;
use tokio::time::{interval_at, timeout, Instant};

type CtrlStream = Box<dyn Duplex>;

/// Starts both loops over a control stream that has passed `Ack`.
pub(super) fn spawn(core: Arc<HyperSessionState>, ctrl: CtrlStream, out: mpsc::UnboundedReceiver<Ctrl>) {
    let (r, w) = tokio::io::split(ctrl);
    tokio::spawn(read_loop(core.clone(), r));
    tokio::spawn(write_loop(core, w, out));
}

async fn read_loop(core: Arc<HyperSessionState>, mut r: ReadHalf<CtrlStream>) {
    let mut closed = core.subscribe();
    loop {
        let read_outcome = tokio::select! {
            biased;
            _ = wait_until_closed(&mut closed) => return,
            read_outcome = timeout(DEAD_AFTER, read_ctrl(&mut r)) => read_outcome,
        };
        if !on_read_outcome(&core, read_outcome) {
            return;
        }
    }
}

/// `false` — the session is closed, the loop ends.
fn on_read_outcome(core: &HyperSessionState, read_outcome: Result<io::Result<Ctrl>, tokio::time::error::Elapsed>) -> bool {
    match read_outcome {
        Ok(Ok(Ctrl::Ping)) => core.send(Ctrl::Pong),
        Ok(Ok(Ctrl::Pong)) => true,
        Ok(Ok(Ctrl::Close { code, reason })) => core.close_remote(code, &reason),
        Ok(Err(_)) => core.close_remote(0, "read ended".as_bytes()),
        Err(_) => core.close_local(CODE_SILENT, "no frames for too long".as_bytes()),
    }
}

async fn write_loop(core: Arc<HyperSessionState>, mut w: WriteHalf<CtrlStream>, mut out: mpsc::UnboundedReceiver<Ctrl>) {
    let mut closed = core.subscribe();
    let mut tick = interval_at(Instant::now() + PING_EVERY, PING_EVERY);
    loop {
        let frame = tokio::select! {
            biased;
            f = out.recv() => f,
            _ = wait_until_closed(&mut closed) => None,
            _ = tick.tick() => Some(Ctrl::Ping),
        };
        if !write_next(&core, &mut w, frame).await {
            break;
        }
    }
    let _ = w.shutdown().await;
}

/// Writes a frame; `false` — no more writing is needed (the last frame, an error or a close).
async fn write_next(core: &HyperSessionState, w: &mut WriteHalf<CtrlStream>, frame: Option<Ctrl>) -> bool {
    let Some(frame) = frame else { return false };
    let last = matches!(frame, Ctrl::Close { .. });
    if write_one(w, &frame).await.is_err() {
        core.close_remote(0, "write failed".as_bytes());
        return false;
    }
    !last
}

async fn write_one(w: &mut WriteHalf<CtrlStream>, frame: &Ctrl) -> io::Result<()> {
    let done = timeout(DEAD_AFTER, async {
        write_ctrl(w, frame).await?;
        w.flush().await
    });
    done.await.unwrap_or_else(|_| Err(io::ErrorKind::TimedOut.into()))
}
