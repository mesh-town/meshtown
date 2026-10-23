//! Why a node is not let in, and how each reason looks on the wire and in the JSON contract.

const WIRE_PREFIX: &str = "code 403: ";

/// Why a node is not let in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deny {
    Left,
    Excluded,
    VotedByMe,
    Blocked,
    /// The node was removed from the network by enough votes.
    Forgotten,
    /// I proposed removing the node and the vote still stands.
    RemovedByMe,
    Unlisted,
    /// Only from the wire: the other side refused by its own vote or block.
    Refused,
}

impl Deny {
    /// The reason sent in the close frame.
    pub fn wire_reason(&self) -> &'static [u8] {
        match self {
            Deny::Left => b"left",
            Deny::Excluded => b"excluded",
            Deny::VotedByMe | Deny::RemovedByMe | Deny::Blocked | Deny::Refused => b"refused",
            Deny::Forgotten => b"removed",
            Deny::Unlisted => b"not admitted",
        }
    }

    /// The word used in the JSON contract.
    pub fn code(&self) -> &'static str {
        match self {
            Deny::Left => "left",
            Deny::Excluded => "excluded",
            Deny::VotedByMe => "voted_by_me",
            Deny::RemovedByMe => "removed_by_me",
            Deny::Blocked => "blocked",
            Deny::Forgotten => "removed",
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
            "removed" => Some(Deny::Forgotten),
            "not admitted" => Some(Deny::Unlisted),
            _ => None,
        }
    }
}
