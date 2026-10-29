//! Shared types of the interface binding facade.

use std::fmt;
use std::io;
use std::sync::{Arc, PoisonError, RwLock};

/// The interface outgoing connections are bound to; `None` means the system routing table.
/// Clones share one value, so a change made through any clone is seen by all of them.
#[derive(Clone, Default)]
pub struct OutIface(Arc<RwLock<Option<Arc<str>>>>);

impl OutIface {
    pub fn new(name: Option<&str>) -> OutIface {
        OutIface(Arc::new(RwLock::new(name.map(Arc::from))))
    }

    /// The current interface name, if one is set.
    pub fn current(&self) -> Option<Arc<str>> {
        self.0
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Replaces the value; returns `true` when it differs from the previous one.
    pub fn set(&self, name: Option<&str>) -> bool {
        let mut slot = self.0.write().unwrap_or_else(PoisonError::into_inner);
        let changed = slot.as_deref() != name;
        if changed {
            *slot = name.map(Arc::from);
        }
        changed
    }
}

/// State of a named interface as seen by the operating system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum IfaceState {
    Up,
    Down,
    Missing,
    Unsupported,
}

/// Why a socket cannot be bound to an interface.
#[derive(Debug)]
pub enum BindError {
    Missing(String),
    Down(String),
    NoPermission(String),
    Unsupported,
    Os(String, io::Error),
}

impl fmt::Display for BindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BindError::Missing(n) => write!(f, "interface {n} is not on this machine"),
            BindError::Down(n) => write!(f, "interface {n} is down"),
            BindError::NoPermission(n) => write!(
                f,
                "binding to interface {n} is not permitted (Linux kernels before 5.7 need CAP_NET_RAW)"
            ),
            BindError::Unsupported => {
                write!(f, "binding to an interface is not supported on this system")
            }
            BindError::Os(n, e) => write!(f, "binding to interface {n} failed: {e}"),
        }
    }
}

impl std::error::Error for BindError {}

/// The exit's interface as shown in the state snapshot.
#[derive(Clone, Debug, serde::Serialize)]
pub struct ExitIfaceInfo {
    pub name: String,
    pub state: IfaceState,
}

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
