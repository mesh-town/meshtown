//! Snapshot selection types: exactly the `GET /api/state` fields that the tray reads.
//!
//! What it does: `TraySnapshot` and the nested structs; each has `#[serde(default)]` and each field has a
//! function from `lenient` — a missing key and a foreign type both give the same default, extra keys are ignored.
//! What it does not do: it does not parse a non-object with a derived `Deserialize` (an array would be read
//! positionally) — outside code uses only `TraySnapshot::from_value`; it does not store the invite string.
//! Links: `lenient.rs` (field readers), `crate::web_gui::RouteMode` (the route as on the server);
//! the node status is its own `TrayNodeStatus`, so that an unknown string does not turn into `Starting`.

use crate::web_gui::RouteMode;
use serde_json::Value;

use super::lenient;

/// The node status word; an unknown one is `Unknown` (in `net_link` that means the connection is lost).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrayNodeStatus {
    Starting,
    Online,
    Offline,
    Stopping,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default)]
pub struct TraySnapshot {
    #[serde(deserialize_with = "lenient::opt_object")]
    pub node: Option<TrayNode>,
    /// The `network` key is `null` (the page listener appends it; with no key there is a network).
    #[serde(rename = "network", deserialize_with = "lenient::is_null")]
    pub network_is_null: bool,
    #[serde(deserialize_with = "lenient::object_or_default")]
    pub route: TrayRoute,
    #[serde(deserialize_with = "lenient::object_or_default")]
    pub exit: TrayOwnExit,
    #[serde(deserialize_with = "lenient::object_or_default")]
    pub socks: TraySocks,
    /// `None` — "peers" is not an array (the ring is empty); `Some(vec![])` — an array with no members.
    #[serde(deserialize_with = "lenient::opt_list")]
    pub peers: Option<Vec<TrayPeer>>,
    #[serde(deserialize_with = "lenient::object_or_default")]
    pub members: TrayMembers,
}

/// The membership facts the tray shows: a running removal vote, requests to join and our own exclusion.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default)]
pub struct TrayMembers {
    /// `members.voting`: how many removal votes are in progress.
    #[serde(deserialize_with = "lenient::u64_or_zero")]
    pub voting: u64,
    /// `members.me_excluded`: this computer was removed from the network.
    #[serde(deserialize_with = "lenient::bool_or_false")]
    pub me_excluded: bool,
    /// `members.admitting`: how many nodes wait for this computer's confirmation to join.
    #[serde(deserialize_with = "lenient::u64_or_zero")]
    pub admitting: u64,
    /// `members.me_pending`: this computer waits for other members to confirm it.
    #[serde(deserialize_with = "lenient::bool_or_false")]
    pub me_pending: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default)]
pub struct TrayNode {
    #[serde(deserialize_with = "lenient::node_word")]
    pub status: Option<TrayNodeStatus>,
    #[serde(deserialize_with = "lenient::string_or_empty")]
    pub id_full: String,
    #[serde(deserialize_with = "lenient::opt_u64")]
    pub uptime_sec: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default)]
pub struct TrayRoute {
    #[serde(deserialize_with = "lenient::route_mode")]
    pub state: RouteMode,
    #[serde(deserialize_with = "lenient::list_or_empty")]
    pub exits: Vec<TrayChosenExit>,
}

/// A selected exit from `route.exits`.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default)]
pub struct TrayChosenExit {
    #[serde(deserialize_with = "lenient::opt_string")]
    pub id: Option<String>,
    #[serde(deserialize_with = "lenient::opt_string")]
    pub id_full: Option<String>,
    #[serde(deserialize_with = "lenient::opt_string")]
    pub name: Option<String>,
    #[serde(deserialize_with = "lenient::opt_string")]
    pub country: Option<String>,
    #[serde(deserialize_with = "lenient::bool_or_false")]
    pub online: bool,
    #[serde(deserialize_with = "lenient::bool_or_false")]
    pub active: bool,
}

/// A member from `peers`.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default)]
pub struct TrayPeer {
    #[serde(deserialize_with = "lenient::opt_string")]
    pub id: Option<String>,
    #[serde(deserialize_with = "lenient::opt_string")]
    pub id_full: Option<String>,
    #[serde(deserialize_with = "lenient::opt_string")]
    pub name: Option<String>,
    #[serde(deserialize_with = "lenient::opt_string")]
    pub country: Option<String>,
    #[serde(rename = "self", deserialize_with = "lenient::bool_or_false")]
    pub is_self: bool,
    #[serde(deserialize_with = "lenient::bool_or_false")]
    pub online: bool,
    #[serde(deserialize_with = "lenient::bool_or_false")]
    pub exit: bool,
    /// Bytes/s of our traffic through this peer as an exit; a missing key gives `None`.
    #[serde(deserialize_with = "lenient::opt_u64")]
    pub through_peer_bps: Option<u64>,
    /// Bytes/s of the peer's traffic through our exit.
    #[serde(deserialize_with = "lenient::opt_u64")]
    pub through_us_bps: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default)]
pub struct TrayOwnExit {
    #[serde(deserialize_with = "lenient::bool_or_false")]
    pub enabled: bool,
    #[serde(deserialize_with = "lenient::strings")]
    pub clients: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default)]
pub struct TraySocks {
    #[serde(deserialize_with = "lenient::bool_or_false")]
    pub enabled: bool,
    #[serde(deserialize_with = "lenient::opt_string")]
    pub listen: Option<String>,
}

impl TraySnapshot {
    /// A selection from the node's reply; a non-object gives an empty selection.
    pub fn from_value(value: &Value) -> Self {
        lenient::from_object(value)
    }
}

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
