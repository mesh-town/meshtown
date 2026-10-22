//! Common port publication types: who can see a port, our own publication, somebody else's port opened here.
//! No logic.

use crate::net::PeerId;
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

/// Our own publication: a port of this device, a name for people and the recipient circle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Published {
    pub port: u16,
    pub name: String,
    pub audience: Audience,
}

/// Somebody else's port opened here: the node, its port and the local listener port.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Opened {
    pub node: PeerId,
    pub port: u16,
    pub local: u16,
}

/// There are no more entries in a publication list or `Nodes` recipients than this.
pub const MAX_PORT_LIST_ENTRIES: usize = 64;
/// A publication name is no longer than this number of characters.
pub const MAX_NAME_CHARS: usize = 64;
