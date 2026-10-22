//! A reconciliation session of one replica over an already open `REPLICA` stream (the caller does the handshake).
//! The frame order is strictly alternating: dialer `Summary` → acceptor `Summary`, `Entries…`, `Done` →
//! dialer `Entries…`, `Done`. The state lock is not held across an `await`.

use super::entry::{check, WireEntry};
use super::state::{State, WriterSum};
use super::wire::{batches, Frame, MAX_SESSION_ENTRIES, PROTO_VERSION, SESSION_TIMEOUT};
use crate::net::error::{NetError, NetResult};
use crate::net::stream::StreamPair;
use crate::sync::flood::wire::MAX_FRAME_BYTES;
use crate::sync::framing::{read_framed, write_framed};
use std::future::Future;
use std::io::{Error, ErrorKind};
use std::sync::{Mutex, MutexGuard};
use tokio::io::{AsyncRead, AsyncWrite};

/// The result of a session from one side: verified entries received, sent, dropped by the check, whether the map changed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SessionTally {
    pub received: u64,
    pub sent: u64,
    pub rejected: u32,
    pub changed: bool,
}

pub(crate) async fn run_dialer(pair: &mut StreamPair, state: &Mutex<State>) -> NetResult<SessionTally> {
    dial(&mut pair.recv, &mut pair.send, state).await
}

pub(crate) async fn run_acceptor(
    pair: &mut StreamPair,
    state: &Mutex<State>,
) -> NetResult<SessionTally> {
    accept(&mut pair.recv, &mut pair.send, state).await
}

pub(crate) async fn dial<R, W>(
    recv: &mut R,
    send: &mut W,
    state: &Mutex<State>,
) -> NetResult<SessionTally>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut side = Side::new(recv, send, state);
    within_timeout(async {
        side.send_summary().await?;
        let theirs = side.read_summary().await?;
        side.read_entries().await?;
        side.send_missing(&theirs).await?;
        Ok(side.tally)
    })
    .await
}

pub(crate) async fn accept<R, W>(
    recv: &mut R,
    send: &mut W,
    state: &Mutex<State>,
) -> NetResult<SessionTally>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut side = Side::new(recv, send, state);
    within_timeout(async {
        let theirs = side.read_summary().await?;
        side.send_summary().await?;
        side.send_missing(&theirs).await?;
        side.read_entries().await?;
        Ok(side.tally)
    })
    .await
}

async fn within_timeout<F: Future<Output = NetResult<SessionTally>>>(run: F) -> NetResult<SessionTally> {
    tokio::time::timeout(SESSION_TIMEOUT, run)
        .await
        .map_err(|_| NetError::Timeout)?
}

fn unexpected() -> NetError {
    NetError::Io(Error::new(ErrorKind::InvalidData, "frame out of order"))
}

struct Side<'a, R, W> {
    recv: &'a mut R,
    send: &'a mut W,
    state: &'a Mutex<State>,
    tally: SessionTally,
    /// How many entries arrived in the session (with dropped ones): the limit `MAX_SESSION_ENTRIES`.
    arrived: usize,
}

impl<'a, R: AsyncRead + Unpin, W: AsyncWrite + Unpin> Side<'a, R, W> {
    fn new(recv: &'a mut R, send: &'a mut W, state: &'a Mutex<State>) -> Self {
        Side {
            recv,
            send,
            state,
            tally: SessionTally::default(),
            arrived: 0,
        }
    }

    fn lock(&self) -> MutexGuard<'a, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    async fn send_summary(&mut self) -> NetResult<()> {
        let writers = self.lock().summary();
        let frame = Frame::Summary {
            version: PROTO_VERSION,
            writers,
        };
        write_framed(self.send, &frame, MAX_FRAME_BYTES).await
    }

    async fn read_summary(&mut self) -> NetResult<Vec<WriterSum>> {
        match read_framed(self.recv, MAX_FRAME_BYTES).await? {
            Frame::Summary { version, writers } if version == PROTO_VERSION => Ok(writers),
            Frame::Summary { .. } => Err(NetError::Refused),
            _ => Err(unexpected()),
        }
    }

    async fn send_missing(&mut self, theirs: &[WriterSum]) -> NetResult<()> {
        let missing = self.lock().missing_for(theirs);
        let total = missing.len();
        for batch in batches(missing) {
            write_framed(self.send, &Frame::Entries(batch), MAX_FRAME_BYTES).await?;
        }
        write_framed(self.send, &Frame::Done { sent: total as u32 }, MAX_FRAME_BYTES).await?;
        self.tally.sent = total as u64;
        Ok(())
    }

    async fn read_entries(&mut self) -> NetResult<()> {
        loop {
            match read_framed(self.recv, MAX_FRAME_BYTES).await? {
                Frame::Entries(batch) => self.absorb(batch)?,
                Frame::Done { .. } => return Ok(()),
                Frame::Summary { .. } => return Err(unexpected()),
            }
        }
    }

    /// Checks and merges a batch under the lock; beyond the session's entry limit — `Refused`.
    fn absorb(&mut self, batch: Vec<WireEntry>) -> NetResult<()> {
        self.arrived += batch.len();
        if self.arrived > MAX_SESSION_ENTRIES {
            return Err(NetError::Refused);
        }
        let mut state = self.lock();
        let id = state.id();
        for w in batch {
            match check(&id, &w) {
                Ok(_) => {
                    self.tally.received += 1;
                    self.tally.changed |= state.merge(w);
                }
                Err(_) => self.tally.rejected += 1,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
