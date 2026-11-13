//! Snapshot structs of the exit node's own state and of the routing decision.

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct ExitInfo {
    pub enabled: bool,
    pub domains: u64,
    /// Full ids (`id_full`) of members that are currently going to the internet through this node: there is an
    /// open exit stream, or it closed less than 30 s ago. Network members only, in
    /// ascending string order; `[]` — no clients or the exit is off (the field is always present).
    pub clients: Vec<String>,
    /// The interface exit connections are bound to and its state; `null` — the system routing table.
    pub interface: Option<crate::iface_bind::ExitIfaceInfo>,
    /// The exit's own load; `null` while the exit of this network is off.
    pub load: Option<super::snapshot_load::ExitLoadInfo>,
}

/// Routing decision for the snapshot: the same selection function the proxy uses
/// (`crate::proxy::route::choose_route`) — here only the result is laid out for the page.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct RouteState {
    pub state: RouteMode,
    pub exits: Vec<ChosenExit>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RouteMode {
    #[default]
    Direct,
    Exit,
    Unavailable,
}

/// One node from `route.exits` (in config order) with its current status.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct ChosenExit {
    pub id: String,
    pub id_full: String,
    pub name: Option<String>,
    pub online: bool,
    pub exit: Option<bool>,
    pub country: Option<String>,
    pub active: bool,
    /// Why the exit says it is overloaded (`limit`, `share`, `memory`, `other`), while its record is counted.
    pub overloaded: Option<String>,
    /// This client's own refusals from the exit over the last two minutes, while they last.
    pub turned_away: Option<super::snapshot_load::TurnedAway>,
    /// `overloaded` or `turned_away` is set: one truth for the page and the tray.
    pub busy: bool,
    /// The node has an exclusion record in the network's verdicts (never set for the own node).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub removed: bool,
}
