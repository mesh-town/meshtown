//! Raw gateway frames: read whole from a stream, classified like the exit does, `conn_id` rewritten in place.

use std::io::{Error, ErrorKind, Result};

use tokio::io::{AsyncRead, AsyncReadExt};

use crate::exit::udpgw::codec::{encode, Frame, FLAG_CLOSE, FLAG_DATA, FLAG_KEEPALIVE};

/// Length field (2).
const LEN_AT: usize = 2;
/// Flags (1) and connection id (2).
const HEADER_LEN: usize = 3;

/// What a frame asks for; checked in the exit's order, so a frame with several flags means the first one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Keepalive,
    Close,
    Data,
    Other,
}

/// One whole frame as it travels on the wire, length field included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RawFrame(Vec<u8>);

/// Reads one whole frame; an end of stream before the first byte is `UnexpectedEof`.
pub(super) async fn read_raw<R: AsyncRead + Unpin>(recv: &mut R) -> Result<RawFrame> {
    let mut len = [0u8; LEN_AT];
    recv.read_exact(&mut len).await?;
    let body_len = usize::from(u16::from_be_bytes(len));
    if body_len < HEADER_LEN {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "frame is shorter than its header",
        ));
    }
    let mut bytes = len.to_vec();
    bytes.resize(LEN_AT + body_len, 0);
    recv.read_exact(&mut bytes[LEN_AT..]).await?;
    Ok(RawFrame(bytes))
}

impl RawFrame {
    /// A frame built by the multiplexer itself (keepalive, error, close); they carry no address.
    pub(super) fn of(frame: &Frame) -> RawFrame {
        RawFrame(encode(frame).unwrap_or_default())
    }

    pub(super) fn close(conn_id: u16) -> RawFrame {
        RawFrame::of(&Frame {
            flags: FLAG_CLOSE,
            conn_id,
            target: None,
            data: Vec::new(),
        })
    }

    pub(super) fn kind(&self) -> Kind {
        let flags = self.0.get(LEN_AT).copied().unwrap_or(0);
        if flags & FLAG_KEEPALIVE != 0 {
            return Kind::Keepalive;
        }
        if flags & FLAG_CLOSE != 0 {
            return Kind::Close;
        }
        if flags & FLAG_DATA != 0 {
            return Kind::Data;
        }
        Kind::Other
    }

    pub(super) fn conn_id(&self) -> u16 {
        let at = LEN_AT + 1;
        match self.0.get(at..at + 2) {
            Some(id) => u16::from_be_bytes([id[0], id[1]]),
            None => 0,
        }
    }

    /// The same frame with another `conn_id`; every other byte stays as it was.
    pub(super) fn with_conn_id(mut self, id: u16) -> RawFrame {
        let at = LEN_AT + 1;
        if let Some(slot) = self.0.get_mut(at..at + 2) {
            slot.copy_from_slice(&id.to_be_bytes());
        }
        self
    }

    /// Everything after the length field.
    pub(super) fn body(&self) -> &[u8] {
        self.0.get(LEN_AT..).unwrap_or_default()
    }

    pub(super) fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
