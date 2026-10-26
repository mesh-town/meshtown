//! Reading the exit's reply to a request: version byte, then the code byte.

use std::io::ErrorKind;

use crate::node::output::short_peer_id;
use crate::transport::net::{BoxRecv, PeerId};
use anyhow::{anyhow, bail, Result};
use tokio::io::AsyncReadExt;

use super::frame::EXIT_PROTOCOL_VERSION;

/// The exit answered with the version byte only and closed the stream: it has no free stream slot. It is
/// carried inside `anyhow::Error`; the caller finds it via `downcast_ref` and tells "exit overloaded" from
/// "exit is not responding".
#[derive(Debug)]
pub struct ExitBusy {
    /// Short exit id.
    pub peer: String,
}

impl std::fmt::Display for ExitBusy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "exit {} has no free stream slot", self.peer)
    }
}

impl std::error::Error for ExitBusy {}

/// Reads the reply code. Nothing at all or a failed read is "reply not received"; a version byte followed by
/// the end of the stream is `ExitBusy`.
pub(super) async fn read_exit_reply(recv: &mut BoxRecv, peer: &PeerId) -> Result<u8> {
    let mut version = [0u8; 1];
    recv.read_exact(&mut version)
        .await
        .map_err(|e| anyhow!("reply not received: {e}"))?;
    if version[0] != EXIT_PROTOCOL_VERSION {
        bail!("unsupported exit reply version: {}", version[0]);
    }
    let mut code = [0u8; 1];
    match recv.read_exact(&mut code).await {
        Ok(_) => Ok(code[0]),
        Err(e) if e.kind() == ErrorKind::UnexpectedEof => Err(ExitBusy {
            peer: short_peer_id(peer),
        }
        .into()),
        Err(e) => Err(anyhow!("reply not received: {e}")),
    }
}
