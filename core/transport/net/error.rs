//! Network errors: `NetError` with hand-written `Display`/`Error` (no thiserror).
//! Only the error type and `NetResult` are here.

use crate::transport::net::id::PeerId;
use std::fmt;

#[derive(Debug)]
pub enum NetError {
    NoRoute,
    WrongPeer { want: PeerId, got: PeerId },
    Refused,
    Closed,
    Timeout,
    Config(String),
    Io(std::io::Error),
    Other(anyhow::Error),
}

pub type NetResult<T> = Result<T, NetError>;

impl fmt::Display for NetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetError::NoRoute => write!(f, "no route to the node"),
            NetError::WrongPeer { want, got } => {
                write!(f, "wrong node at the other end: expected {want}, got {got}")
            }
            NetError::Refused => write!(f, "connection refused"),
            NetError::Closed => write!(f, "connection closed"),
            NetError::Timeout => write!(f, "timed out"),
            NetError::Config(msg) => write!(f, "network configuration error: {msg}"),
            NetError::Io(e) => write!(f, "I/O error: {e}"),
            NetError::Other(e) => write!(f, "{e}"),
        }
    }
}

impl NetError {
    /// Into `anyhow`: `Other` returns the inner error as is, the others are wrapped whole.
    pub fn into_anyhow(self) -> anyhow::Error {
        match self {
            NetError::Other(e) => e,
            other => anyhow::Error::new(other),
        }
    }
}

impl std::error::Error for NetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            NetError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for NetError {
    fn from(e: std::io::Error) -> Self {
        NetError::Io(e)
    }
}
