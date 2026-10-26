//! Common port publication types: who can see a port, our own publication, somebody else's port opened here.
//! The only logic is the protocol table.

use crate::transport::net::PeerId;
use serde::{Deserialize, Serialize};

/// Who can see a published port. The order of variants is the variant number in the document entry;
/// only append at the end.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Audience {
    /// All members of the network.
    All,
    /// Only the listed nodes.
    Nodes(Vec<PeerId>),
}

/// Transport protocol of a published or opened port. The discriminant is the table index.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Proto {
    #[default]
    Tcp = 0,
    Udp = 1,
}

const PROTO_NAMES: [&str; 2] = ["tcp", "udp"];

impl Proto {
    /// Every protocol, in discriminant order.
    pub const ALL: [Proto; 2] = [Proto::Tcp, Proto::Udp];

    /// `"tcp"` or `"udp"`.
    pub fn as_str(self) -> &'static str {
        PROTO_NAMES[self as usize]
    }

    /// Exact `"tcp"` / `"udp"` only.
    pub fn parse(s: &str) -> Option<Proto> {
        Proto::ALL.into_iter().find(|p| p.as_str() == s)
    }

    /// The byte of the protocol in a request frame.
    pub fn wire(self) -> u8 {
        self as u8
    }

    /// The protocol of a frame byte; unknown bytes give `None`.
    pub fn from_wire(b: u8) -> Option<Proto> {
        Proto::ALL.into_iter().find(|p| p.wire() == b)
    }

    /// For `skip_serializing_if`: TCP is the default and is not written.
    pub fn is_tcp(&self) -> bool {
        *self == Proto::Tcp
    }

    /// Chooses by protocol; the only place that branches on `Proto`.
    pub fn pick<T>(self, tcp: T, udp: T) -> T {
        match self {
            Proto::Tcp => tcp,
            Proto::Udp => udp,
        }
    }
}

/// Our own publication: a port of this device, a name for people and the recipient circle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Published {
    pub port: u16,
    pub proto: Proto,
    pub name: String,
    pub audience: Audience,
}

/// Somebody else's port opened here: the node, its port and the local listener port.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Opened {
    pub node: PeerId,
    pub port: u16,
    pub proto: Proto,
    pub local: u16,
}

/// There are no more entries in a publication list or `Nodes` recipients than this.
pub const MAX_PORT_LIST_ENTRIES: usize = 64;
/// A publication name is no longer than this number of characters.
pub const MAX_NAME_CHARS: usize = 64;

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
