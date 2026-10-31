//! How an attempt to open the first lane of an exit failed, in a form every waiting connection can get.

use anyhow::{anyhow, Result};
use tokio::io::{duplex, split};

use crate::exit;
use crate::transport::net::{BoxRecv, BoxSend};

/// A failed attempt to open the first lane, in a form that can be handed to every waiter.
#[derive(Clone)]
pub(super) enum Failure {
    ExitBusy(String),
    StreamBusy {
        peer: String,
        secs: u64,
        waiters: usize,
    },
    Failed(String),
    Code(u8),
}

impl Failure {
    pub(super) fn of(e: &anyhow::Error) -> Failure {
        if let Some(busy) = e.downcast_ref::<exit::ExitBusy>() {
            return Failure::ExitBusy(busy.peer.clone());
        }
        if let Some(busy) = e.downcast_ref::<exit::StreamBusy>() {
            let (peer, secs, waiters) = (busy.peer.clone(), busy.secs, busy.open_bi_waiter_count);
            return Failure::StreamBusy {
                peer,
                secs,
                waiters,
            };
        }
        Failure::Failed(format!("{e:#}"))
    }

    pub(super) fn answer(self) -> Result<(BoxSend, BoxRecv, u8)> {
        match self {
            Failure::ExitBusy(peer) => Err(exit::ExitBusy { peer }.into()),
            Failure::StreamBusy {
                peer,
                secs,
                waiters,
            } => Err(exit::StreamBusy {
                peer,
                secs,
                open_bi_waiter_count: waiters,
            }
            .into()),
            Failure::Failed(text) => Err(anyhow!(text)),
            Failure::Code(code) => Ok(closed_pair(code)),
        }
    }
}

/// A pair that is already closed, for a waiter whose exit refused with `code`.
fn closed_pair(code: u8) -> (BoxSend, BoxRecv, u8) {
    let (end, other) = duplex(1);
    drop(other);
    let (read, write) = split(end);
    (Box::new(write), Box::new(read), code)
}
