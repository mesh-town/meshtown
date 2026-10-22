//! A `mem` connection frame: one frame per packet, big-endian fields, the first byte is the kind (`0xFF` is not taken).
//! The handshake, streams and closing are in `carrier` and `conn`.

use bytes::{BufMut, Bytes, BytesMut};
use std::fmt;

const KIND_HELLO: u8 = 0x01;
const KIND_ACCEPT: u8 = 0x02;
const KIND_REFUSE: u8 = 0x03;
const KIND_OPEN: u8 = 0x10;
const KIND_DATA: u8 = 0x11;
const KIND_FIN: u8 = 0x12;
const KIND_WINDOW: u8 = 0x13;
const KIND_CLOSE: u8 = 0x20;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Frame {
    Hello(Bytes),
    Accept,
    Refuse,
    Open(u32),
    Data(u32, Bytes),
    Fin(u32),
    Window(u32, u32),
    Close { code: u32, reason: Bytes },
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum FrameError {
    Empty,
    UnknownKind(u8),
    Short,
    Trailing,
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameError::Empty => write!(f, "empty frame"),
            FrameError::UnknownKind(k) => write!(f, "unknown frame kind {k:#04x}"),
            FrameError::Short => write!(f, "frame shorter than its header"),
            FrameError::Trailing => write!(f, "extra bytes in the frame"),
        }
    }
}

impl std::error::Error for FrameError {}

fn with_id(kind: u8, id: u32, extra: usize) -> BytesMut {
    let mut b = BytesMut::with_capacity(5 + extra);
    b.put_u8(kind);
    b.put_u32(id);
    b
}

impl Frame {
    pub(crate) fn encode(&self) -> Bytes {
        match self {
            Frame::Hello(name) => tagged(KIND_HELLO, name),
            Frame::Accept => tagged(KIND_ACCEPT, &[]),
            Frame::Refuse => tagged(KIND_REFUSE, &[]),
            Frame::Open(id) => with_id(KIND_OPEN, *id, 0).freeze(),
            Frame::Data(id, data) => {
                let mut b = with_id(KIND_DATA, *id, data.len());
                b.extend_from_slice(data);
                b.freeze()
            }
            Frame::Fin(id) => with_id(KIND_FIN, *id, 0).freeze(),
            Frame::Window(id, inc) => {
                let mut b = with_id(KIND_WINDOW, *id, 4);
                b.put_u32(*inc);
                b.freeze()
            }
            Frame::Close { code, reason } => {
                let mut b = with_id(KIND_CLOSE, *code, reason.len());
                b.extend_from_slice(reason);
                b.freeze()
            }
        }
    }

    pub(crate) fn decode(mut raw: Bytes) -> Result<Frame, FrameError> {
        let kind = *raw.first().ok_or(FrameError::Empty)?;
        let body = raw.split_off(1);
        decode_body(kind, body)
    }
}

fn tagged(kind: u8, body: &[u8]) -> Bytes {
    let mut b = BytesMut::with_capacity(1 + body.len());
    b.put_u8(kind);
    b.extend_from_slice(body);
    b.freeze()
}

fn take_u32(body: &Bytes, at: usize) -> Result<u32, FrameError> {
    let raw = body.get(at..at + 4).ok_or(FrameError::Short)?;
    Ok(u32::from_be_bytes(
        raw.try_into().map_err(|_| FrameError::Short)?,
    ))
}

/// Exactly `len` bytes of body: fewer — `Short`, more — `Trailing`.
fn exact(body: &Bytes, len: usize) -> Result<(), FrameError> {
    if body.len() < len {
        return Err(FrameError::Short);
    }
    if body.len() > len {
        return Err(FrameError::Trailing);
    }
    Ok(())
}

fn decode_body(kind: u8, body: Bytes) -> Result<Frame, FrameError> {
    match kind {
        KIND_HELLO if body.is_empty() => Err(FrameError::Short),
        KIND_HELLO => Ok(Frame::Hello(body)),
        KIND_ACCEPT => exact(&body, 0).map(|_| Frame::Accept),
        KIND_REFUSE => exact(&body, 0).map(|_| Frame::Refuse),
        KIND_OPEN => exact(&body, 4).and_then(|_| Ok(Frame::Open(take_u32(&body, 0)?))),
        KIND_FIN => exact(&body, 4).and_then(|_| Ok(Frame::Fin(take_u32(&body, 0)?))),
        KIND_WINDOW => exact(&body, 8)
            .and_then(|_| Ok(Frame::Window(take_u32(&body, 0)?, take_u32(&body, 4)?))),
        KIND_DATA => Ok(Frame::Data(take_u32(&body, 0)?, body.slice(4..))),
        KIND_CLOSE => Ok(Frame::Close {
            code: take_u32(&body, 0)?,
            reason: body.slice(4..),
        }),
        other => Err(FrameError::UnknownKind(other)),
    }
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
