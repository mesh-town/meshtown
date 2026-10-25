//! Replica reconciliation sessions: a dial to a peer (`dial`), accepting (`accept`), the result is the `Synced` event.

use super::replica::MeshReplica;
use super::replica_sync::SessionGuard;
use super::session::{run_acceptor, run_dialer, SessionTally};
use crate::members::sync::flood::admit::dial_handshake;
use crate::members::sync::flood::wire::{CLOSE_BAD_FRAME, CLOSE_REJECTED, CLOSE_SHUTDOWN};
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::id::PeerId;
use crate::transport::net::replica::{ReplicaEvent, SyncCount};
use crate::transport::net::stream::StreamPair;
use crate::transport::net::transport::PeerConn;
use crate::transport::net::REPLICA;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::time::timeout;

/// The timeout for the connection, stream and handshake of a dial.
const HANDSHAKE: Duration = Duration::from_secs(5);
/// How long the dialer waits for the acceptor to close the link after the last frame.
const CLOSE_WAIT: Duration = Duration::from_secs(2);

impl MeshReplica {
    /// A session on `NeighborUp`, `Lagged` or `sync_with`; the `slot` is freed before the event.
    pub(super) async fn dial_session(self: &Arc<Self>, peer: PeerId, slot: SessionGuard) {
        let before = self.lock().generation();
        let res = self.dial(peer).await;
        drop(slot);
        self.finish(peer, before, res);
    }

    /// The dial: `REPLICA` connection → stream → handshake → reconciliation → close.
    async fn dial(&self, peer: PeerId) -> NetResult<SessionTally> {
        let stack = self
            .wiring
            .stack
            .get()
            .and_then(|s| s.upgrade())
            .ok_or(NetError::Closed)?;
        let conn = stack.connect(peer, REPLICA).await?;
        let handshake = timeout(HANDSHAKE, self.open_session(&conn, peer));
        let opened = handshake.await.unwrap_or(Err(NetError::Timeout));
        let mut pair = opened.inspect_err(|_| conn.close(CLOSE_REJECTED, b"handshake"))?;
        let res = run_dialer(&mut pair, &self.state).await;
        let _ = pair.send.shutdown().await;
        self.hang_up(&conn, res.is_ok()).await;
        res
    }

    async fn open_session(&self, conn: &Arc<dyn PeerConn>, peer: PeerId) -> NetResult<StreamPair> {
        let mut pair = conn.open().await?;
        dial_handshake(
            &mut pair,
            &self.wiring.admission_key,
            self.wiring.me,
            peer,
            self.id.as_bytes(),
        )
        .await?;
        Ok(pair)
    }

    /// The dialer writes last: lets the acceptor finish reading and close the link, then closes itself.
    async fn hang_up(&self, conn: &Arc<dyn PeerConn>, clean: bool) {
        if clean {
            let _ = timeout(CLOSE_WAIT, conn.closed()).await;
        }
        let code = if clean {
            CLOSE_SHUTDOWN
        } else {
            CLOSE_BAD_FRAME
        };
        conn.close(code, b"sync done");
    }

    /// The accepting side: the handshake has already been done by the service.
    pub(super) async fn accept(
        self: Arc<Self>,
        peer: PeerId,
        conn: Arc<dyn PeerConn>,
        mut pair: StreamPair,
    ) {
        let before = self.lock().generation();
        let res = run_acceptor(&mut pair, &self.state).await;
        let code = if res.is_ok() {
            CLOSE_SHUTDOWN
        } else {
            CLOSE_BAD_FRAME
        };
        conn.close(code, b"sync done");
        self.finish(peer, before, res);
    }

    /// End of session: saving, `RemoteChange` if the map changed, `Synced`. `generation_before` is the map generation before the session:
    /// a session that broke off after merging some of the batches is also saved.
    pub(super) fn finish(
        &self,
        peer: PeerId,
        generation_before: u64,
        session_result: NetResult<SessionTally>,
    ) {
        self.save_after_session(generation_before, &session_result);
        self.log_session_end(peer, session_result);
    }

    /// A failed session that merged some batches still announces the change and saves.
    fn save_after_session(&self, generation_before: u64, session_result: &NetResult<SessionTally>) {
        if session_result.is_err() && self.lock().generation() > generation_before {
            self.emit(ReplicaEvent::RemoteChange);
            self.saver.save_debounced(self.state.clone());
        }
    }

    /// Logs a failed session and emits `Synced` with its outcome.
    fn log_session_end(&self, peer: PeerId, session_result: NetResult<SessionTally>) {
        let outcome = session_result
            .map(|x| self.apply_exchange(peer, x))
            .map_err(|e| e.to_string());
        if let Err(text) = &outcome {
            tracing::warn!(
                "[sync] reconciliation of {} with {peer} failed: {text}",
                self.id.fmt_short()
            );
        }
        self.emit(ReplicaEvent::Synced { peer, outcome });
    }

    fn apply_exchange(&self, peer: PeerId, tally: SessionTally) -> SyncCount {
        if tally.rejected > 0 {
            tracing::warn!(
                "[sync] dropped {} entries from {peer}: bad signature or limit",
                tally.rejected
            );
        }
        if tally.changed {
            self.emit(ReplicaEvent::RemoteChange);
            self.saver.save_debounced(self.state.clone());
        }
        SyncCount {
            received: tally.received,
            sent: tally.sent,
        }
    }
}
