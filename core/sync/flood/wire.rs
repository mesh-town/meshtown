//! The `SYNC` wire: frame kinds, link close codes, writing and reading a frame (u32 BE length + postcard).
//! There is no admission or peer state here.

use crate::net::error::NetResult;
use crate::net::stream::{BoxRecv, BoxSend};
use serde::{Deserialize, Serialize};
use crate::sync::framing::{read_framed, write_framed};

pub const VERSION: u16 = 1;
pub const MAX_FRAME_BYTES: usize = 64 * 1024;
pub const MAX_BODY_BYTES: usize = 60 * 1024;

pub const CLOSE_DUPLICATE: u32 = 10;
pub const CLOSE_REJECTED: u32 = 11;
pub const CLOSE_BAD_FRAME: u32 = 12;
pub const CLOSE_IDLE: u32 = 13;
pub const CLOSE_SLOW: u32 = 14;
pub const CLOSE_SHUTDOWN: u32 = 15;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Frame {
    /// The dialer goes first: version, topic, admission tag.
    Hello {
        version: u16,
        topic: [u8; 32],
        tag: [u8; 32],
    },
    /// The acceptor in reply: its admission tag.
    Welcome { tag: [u8; 32] },
    /// A message: the remaining hops and the body.
    Msg { ttl: u8, body: Vec<u8> },
    /// Only resets the silence counter.
    Ping,
}

pub async fn write_frame(send: &mut BoxSend, frame: &Frame) -> NetResult<()> {
    write_framed(send, frame, MAX_FRAME_BYTES).await
}

/// A length above the limit or a corrupt body is `Io(InvalidData)`; end of stream is `Closed`.
pub async fn read_frame(recv: &mut BoxRecv) -> NetResult<Frame> {
    read_framed(recv, MAX_FRAME_BYTES).await
}

#[cfg(test)]
#[path = "wire_tests.rs"]
mod tests;
