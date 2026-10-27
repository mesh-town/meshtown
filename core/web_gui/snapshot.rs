//! `NodeSnapshot` — the whole node as one value for the page (JSON) — and nested structs.

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct NodeSnapshot {
    pub boot_id: String,
    pub now: f64,
    pub node: NodeRuntime,
    pub members: MembersInfo,
    pub socks: SocksInfo,
    pub exit: ExitInfo,
    pub transport: TransportInfo,
    pub peers: Vec<PeerInfo>,
    pub domains: Vec<DomainInfo>,
    pub route: RouteState,
    pub ports: PortsInfo,
    pub log: LogSummary,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct NodeRuntime {
    pub status: NodeStatus,
    pub id: String,
    pub id_full: String,
    pub uptime_sec: u64,
    pub rss_kib: u64,
    pub rss_peak_kib: u64,
    /// Why the last start failed while the node waits for a network; `null` otherwise.
    pub start_error: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeStatus {
    #[default]
    Starting,
    Online,
    Offline,
    Stopping,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct MembersInfo {
    pub count: u64,
    pub gossip_neighbors: u64,
    /// This node has been excluded or has left.
    pub me_excluded: bool,
    /// Requests to join that wait for this node's confirmation.
    pub admitting: u64,
    /// This node waits for others to confirm it.
    pub me_pending: bool,
    /// This computer is kept out: no other member is online and one of them refused it (see `docs/WEB-API.md`).
    pub admission_refused: bool,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct SocksInfo {
    pub enabled: bool,
    /// Address for this machine: always `127.0.0.1:<port>`, in both access modes.
    pub listen: Option<String>,
    /// `socks.access`: `"localhost"` or `"localnet"`.
    pub access: String,
    /// Addresses for the network's other devices (`{addr, iface}`, default-route address first); `[]` under `localhost`.
    pub lan: Vec<SocksLan>,
    /// Effective proxy network filter (`socks.ifaces`): `null` — automatic, `["*"]` — all.
    pub ifaces: Option<Vec<String>>,
    /// PAC address on the proxy port; `None` — the proxy is off.
    pub pac_url: Option<String>,
}

/// Proxy address for the network's other devices and the name of the interface it belongs to.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct SocksLan {
    pub addr: String,
    pub iface: String,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct ExitInfo {
    pub enabled: bool,
    pub domains: u64,
    /// Full ids (`id_full`) of members that are currently going to the internet through this node: there is an
    /// open exit stream, or it closed less than 30 s ago. Network members only, in
    /// ascending string order; `[]` — no clients or the exit is off (the field is always present).
    pub clients: Vec<String>,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct TransportInfo {
    pub self_path: Option<PathKindView>,
    pub relay: RelayInfo,
    pub nat: String,
    pub listen_udp: Vec<String>,
    pub dht: DhtInfo,
    pub record: RecordInfo,
    pub self_addresses: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PathKindView {
    #[default]
    Direct,
    Relay,
}

impl PathKindView {
    /// Path kind for the page from the network layer's path kind; `Unknown` — there is no path yet.
    pub fn from_net(p: &crate::transport::net::PathKind) -> Option<Self> {
        match p {
            crate::transport::net::PathKind::Direct => Some(Self::Direct),
            crate::transport::net::PathKind::Relayed(_) => Some(Self::Relay),
            crate::transport::net::PathKind::Unknown => None,
        }
    }
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct RelayInfo {
    pub connected: bool,
    pub url: Option<String>,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct DhtInfo {
    pub joined: bool,
    pub nodes: u64,
    pub last_publish_sec: Option<u64>,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct RecordInfo {
    pub published: bool,
    pub ttl_sec: u64,
    /// Seconds since the entry was last read from HyperDHT; `None` — never read.
    pub read_sec: Option<u64>,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct PeerInfo {
    pub id: String,
    pub id_full: String,
    pub name: Option<String>,
    #[serde(rename = "self")]
    pub is_self: bool,
    pub online: bool,
    pub path: Option<PathKindView>,
    pub rtt_ms: Option<u64>,
    pub exit: Option<bool>,
    pub country: Option<String>,
    /// Member's program version (`node/version/`); `null` — the member didn't announce it (old
    /// build, the record hasn't arrived yet).
    #[serde(rename = "version")]
    pub program_version: Option<String>,
    pub checked: u64,
    pub stale: bool,
    pub last_seen_sec: u64,
    /// Our traffic through this peer as an exit, bytes/s over 10 s (sent + received); own row and no data — 0.
    pub through_peer_bps: u64,
    /// This peer's traffic through our exit, bytes/s over 10 s; own row and no data — 0.
    pub through_us_bps: u64,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct DomainInfo {
    pub name: String,
    pub checks: std::collections::BTreeMap<String, Option<CheckInfo>>,
    pub checking: bool,
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
    /// The node has an exclusion record in the network's verdicts (never set for the own node).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub removed: bool,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct CheckInfo {
    pub verdict: VerdictKind,
    pub reason: String,
    pub dns_ms: u64,
    pub tcp_ms: u64,
    pub tls_ms: u64,
    pub http_ms: u64,
    pub status: u16,
    pub bytes: u64,
    pub stalled: bool,
    pub age_sec: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerdictKind {
    #[default]
    Ok,
    DnsMismatch,
    NetworkBlocked,
    SiteGeoblock,
    Antibot,
    Down,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct LogSummary {
    pub tags: Vec<String>,
    pub ring_lines: u64,
    pub first_seq: u64,
    pub last_seq: u64,
    pub tail: Vec<LogLine>,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct LogLine {
    pub seq: u64,
    pub t: f64,
    pub tag: String,
    pub text: String,
    /// The label of the network the line belongs to; not part of the JSON of this struct.
    #[serde(skip)]
    pub net: Option<std::sync::Arc<str>>,
    /// Whether the label is displayed (more than one network when the line was written).
    #[serde(skip)]
    pub shown: bool,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct LogPage {
    pub boot_id: String,
    pub first_seq: u64,
    pub last_seq: u64,
    pub lines: Vec<LogLine>,
}

/// Port publishing: own publications and network ports.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct PortsInfo {
    pub published: Vec<OwnPort>,
    pub network: Vec<NetPort>,
}

/// Own publication; `local` is `"unknown"`, `"ok"` or `"refused"`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct OwnPort {
    pub port: u16,
    pub proto: crate::ports_sharing::Proto,
    pub name: String,
    pub local: String,
}

/// A port of another node. `published: false` — open here, but there is no valid `All` record.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct NetPort {
    pub node: String,
    pub node_name: Option<String>,
    pub online: bool,
    pub port: u16,
    pub proto: crate::ports_sharing::Proto,
    pub name: String,
    pub published: bool,
    pub opened: Option<OpenedInfo>,
    /// Link to the port at the member's mesh address; `None` while the port is not shared with this node
    /// or there is no proxy to answer the address.
    pub web_url: Option<String>,
}

/// A port open here; `state` is `"listening"` or `"bind_failed"`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct OpenedInfo {
    pub local: u16,
    pub state: String,
    pub error: Option<String>,
    /// Outcome of the last failed attempt to reach the publisher; `None` after a success or before any attempt.
    pub last_error: Option<crate::ports_sharing::PortReach>,
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "snapshot_path_kind_tests.rs"]
mod path_kind_tests;
