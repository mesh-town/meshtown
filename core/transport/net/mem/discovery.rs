//! `mem` discovery: a board of entries per (hub, name) and the `MemDiscovery` source on top of it.
//! Merging of sources is in the core (`discovery_mux`).

use crate::transport::net::addr::PeerAddr;
use crate::transport::net::discovery::Discovery;
use crate::transport::net::error::NetResult;
use crate::transport::net::id::PeerId;
use crate::transport::net::stream::BoxFuture;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::sync::{broadcast, mpsc};

const BOARD_QUEUE: usize = 256;

/// Entries of one source name in one hub.
pub(super) struct Board {
    map: Mutex<HashMap<PeerId, PeerAddr>>,
    tx: broadcast::Sender<PeerAddr>,
}

impl Default for Board {
    fn default() -> Self {
        Board {
            map: Mutex::default(),
            tx: broadcast::channel(BOARD_QUEUE).0,
        }
    }
}

impl Board {
    fn lock(&self) -> MutexGuard<'_, HashMap<PeerId, PeerAddr>> {
        self.map.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// An entry replaces the previous one with the same id and is broadcast.
    fn put(&self, addr: &PeerAddr) {
        let mut map = self.lock();
        map.insert(addr.id, addr.clone());
        let _ = self.tx.send(addr.clone());
    }

    fn snapshot(&self) -> Vec<PeerAddr> {
        self.lock().values().cloned().collect()
    }
}

pub struct MemDiscovery {
    name: &'static str,
    publishes: bool,
    board: Arc<Board>,
}

/// Puts an address into `out`; `false` means the receiver is closed.
async fn push(out: &mpsc::Sender<PeerAddr>, addr: PeerAddr) -> bool {
    tokio::select! {
        r = out.send(addr) => r.is_ok(),
        _ = out.closed() => false,
    }
}

async fn push_all(out: &mpsc::Sender<PeerAddr>, board: &Board) -> bool {
    for addr in board.snapshot() {
        if !push(out, addr).await {
            return false;
        }
    }
    true
}

/// One broadcast step: `false` means the work is over.
async fn step(
    out: &mpsc::Sender<PeerAddr>,
    board: &Board,
    got: Result<PeerAddr, broadcast::error::RecvError>,
) -> bool {
    match got {
        Ok(addr) => push(out, addr).await,
        Err(broadcast::error::RecvError::Lagged(_)) => push_all(out, board).await,
        Err(broadcast::error::RecvError::Closed) => false,
    }
}

async fn run_board(board: Arc<Board>, out: mpsc::Sender<PeerAddr>) {
    let mut sub = board.tx.subscribe();
    if !push_all(&out, &board).await {
        return;
    }
    loop {
        let got = tokio::select! {
            g = sub.recv() => g,
            _ = out.closed() => return,
        };
        if !step(&out, &board, got).await {
            return;
        }
    }
}

impl MemDiscovery {
    pub(super) fn new(name: &'static str, publishes: bool, board: Arc<Board>) -> MemDiscovery {
        MemDiscovery {
            name,
            publishes,
            board,
        }
    }
}

impl Discovery for MemDiscovery {
    fn name(&self) -> &'static str {
        self.name
    }

    fn publishes(&self) -> bool {
        self.publishes
    }

    fn publish(&self, own_addr: &PeerAddr) -> BoxFuture<'_, NetResult<()>> {
        let own_addr = own_addr.clone();
        Box::pin(async move {
            if self.publishes {
                self.board.put(&own_addr);
            }
            Ok(())
        })
    }

    fn run_source(self: Arc<Self>, out: mpsc::Sender<PeerAddr>) -> BoxFuture<'static, ()> {
        Box::pin(run_board(self.board.clone(), out))
    }
}

#[cfg(test)]
#[path = "discovery_tests.rs"]
mod tests;
