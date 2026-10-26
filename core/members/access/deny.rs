//! Why a node is not let in, and how each reason looks on the wire and in the JSON contract.

const WIRE_PREFIX: &str = "code 403: ";

/// Why a node is not let in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deny {
    /// I left the network myself.
    Left,
    Excluded,
    Blocked,
    Unlisted,
    /// Only from the wire: the other side refused by its own block.
    Refused,
}

impl Deny {
    /// The reason sent in the close frame.
    pub fn wire_reason(&self) -> &'static [u8] {
        match self {
            Deny::Left => b"left",
            Deny::Excluded => b"excluded",
            Deny::Blocked | Deny::Refused => b"refused",
            Deny::Unlisted => b"not admitted",
        }
    }

    /// The word used in the JSON contract.
    pub fn code(&self) -> &'static str {
        match self {
            Deny::Left => "left",
            Deny::Excluded => "excluded",
            Deny::Blocked => "blocked",
            Deny::Unlisted => "unlisted",
            Deny::Refused => "refused",
        }
    }

    /// Parses `code 403: <reason>` as reported by `PeerConn::close_reason`.
    pub fn from_wire(text: &str) -> Option<Deny> {
        match text.strip_prefix(WIRE_PREFIX)? {
            "left" => Some(Deny::Left),
            "excluded" => Some(Deny::Excluded),
            "refused" => Some(Deny::Refused),
            "not admitted" => Some(Deny::Unlisted),
            _ => None,
        }
    }
}
