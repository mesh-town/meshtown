/* Node API types: enums, the `GET /api/state` snapshot, the `GET /api/log` log. Types only; the shared header and rules are in `api-types.ts`. */

// ---------------------------------------------------------------- shared enums

/** `node.status` (`NodeStatus`, `snapshot.rs`). */
export type NodeStatus = "starting" | "online" | "offline" | "stopping";

/** Path to a member and the node's own path to the network (`PathKind`). */
export type PathKind = "direct" | "relay";

/** Transport mode (`TransportMode::as_str`, `config_file/settings.rs`). */
export type TransportMode = "direct_and_relay" | "relay_only" | "no_relay";

/** Domain check verdict (`VerdictKind`, snake_case). */
export type Verdict = "ok" | "dns_mismatch" | "network_blocked" | "site_geoblock" | "antibot" | "down";

/** `route.state` (`RouteMode`). */
export type RouteMode = "direct" | "exit" | "unavailable";

/** `node.identity` and `node.identity_reason` (`ConfigState::identity`, `config_api/state.rs`). */
export type Identity = "persistent" | "temporary";
export type IdentityReason = "declined" | "file_preserved" | "file_problem";

/** State of the config file (`FileState::as_str`, `config_file/report.rs`). */
export type ConfigFileState = "ok" | "created" | "unreadable" | "invalid";

// ---------------------------------------------------------------- GET /api/state

/** The whole node snapshot, `GET /api/state` — docs/WEB-API.md §1.3. */
export interface NodeState {
  boot_id: string; // 8 hex characters, new on every process start
  now: number; // seconds since the node started, one decimal place
  config_rev: number;
  node: NodeRuntime;
  /** `null` — there is no network yet (first run) or the key did not parse. */
  network: NetworkInfo | null;
  members: MembersInfo;
  socks: SocksInfo;
  exit: ExitInfo;
  transport: TransportInfo;
  peers: PeerInfo[];
  domains: DomainInfo[];
  route: RouteState;
  ports: PortsInfo;
  log: LogSummary;
  /** Node page: access and the effective network filter; no key — the node is older. */
  web?: WebState;
  /** Paths that are saved but not in effect yet; `"socks.ifaces"` — a restart is needed (`POST /api/actions/restart`). */
  restart_pending?: string[];
}

/** Who decides whether a network is open: auto — automatic (on a router it closes the ISP network, private ones are open);
 *  list — the ifaces list (including `["*"]`); none — the "this computer only" mode or the side is off. */
export type NetBy = "auto" | "list" | "none";

/** One machine network in `node.networks`. */
export interface NetEntry {
  name: string;
  addrs: string[]; // IPv4 without a port; [] for a missing one
  default_route: boolean; // default route — on a router, the ISP network
  kind?: "tunnel" | "virtual"; // by name: a VPN tunnel or Docker/virtual machine; no key — an ordinary network
  present: boolean; // false — the name is in the list but there is no such interface on the machine now
  open: boolean;
  by: NetBy;
}

/** The `web` block of the snapshot. */
export interface WebState {
  access: "localhost" | "localnet";
  listening: "localhost" | "localnet"; // ≠ access — the listener is on the loopback until a restart
  ifaces: string[] | null; // effective filter: null — auto, ["*"] — all
}

export interface NodeRuntime {
  status: NodeStatus;
  id: string; // short id
  id_full: string;
  uptime_sec: number;
  rss_kib: number;
  rss_peak_kib: number;
  // The fields below are inserted by `apply_state_extras` (`routes/state.rs`) on top of the serde struct.
  version: string;
  web_addr: string; // "127.0.0.1:<port>"
  identity: Identity;
  /** `null` when `identity: "persistent"`. */
  identity_reason: IdentityReason | null;
  /** Joining by a `mesh-invite2-` invite: waiting or refused; `null` — not in progress. */
  invite_join?: InviteJoinInfo | null;
  /** Automatic mode found the router (forwarding is on, there is a second private network). The fields below are optional: the node is older. */
  router?: boolean;
  /** The network of this request; `null` — the page is open on the machine itself. Do not clear the via_iface checkbox (otherwise 422). */
  via_iface?: string | null;
  /** `open`/`by` for the `networks_side` side; no loopback, tunnels and virtual ones come with `kind`. */
  networks?: NetEntry[];
  networks_side?: "socks" | "web" | "none"; // whose state is shown in `networks`
}

/** `node.invite_join`: state of joining by a single-use invite. */
export interface InviteJoinInfo {
  state: "waiting" | "refused";
  message: string;
  label: string | null;
  expires_unix: number;
}

/** `network` of the snapshot (`ConfigState::network_block`). */
export interface NetworkInfo {
  label: string | null; // key label; for a key without a label — null
  masked: string;
  id_hex: string;
}

export interface MembersInfo {
  count: number;
  gossip_neighbors: number;
  voting?: number; // rows with a pending ban vote
  me_excluded?: boolean; // this node is no longer in the member list
  admitting?: number; // join requests that wait for this node's confirmation
  me_pending?: boolean; // this node waits for members to confirm it
}

export interface SocksInfo {
  enabled: boolean;
  listen: string | null; // address for this machine (`127.0.0.1:port`), including one set by a flag
  access?: "localhost" | "localnet"; // `socks.access`
  lan?: { addr: string; iface: string }[]; // addresses for other devices on the network; [] — the proxy is closed to the network
  ifaces?: string[] | null; // effective filter: null — auto, ["*"] — all; the saved one — GET /api/config
  pac_url: string | null; // PAC address on the proxy port ("http://127.0.0.1:1080/proxy.pac"); null — the proxy is off
}

export interface ExitInfo {
  enabled: boolean;
  domains: number;
  /** Full ids of the members going to the internet through this node; `[]` — nobody. */
  clients: string[];
}

export interface TransportInfo {
  mode: string; // actually TransportMode, but it is a String in `snapshot.rs`
  self_path: PathKind | null;
  relay: RelayInfo;
  nat: string; // text in English
  listen_udp: string[];
  dht: DhtInfo;
  record: RecordInfo;
  self_addresses: string[]; // "<id>@<address>" — the same as GET /api/peer-addresses
}

export interface RelayInfo {
  connected: boolean;
  url: string | null;
}

export interface DhtInfo {
  joined: boolean;
  nodes: number;
  last_publish_sec: number | null;
}

export interface RecordInfo {
  published: boolean;
  ttl_sec: number;
  /** Seconds since the last read of the record from HyperDHT; `null` — not read. */
  read_sec: number | null;
}

export interface PeerInfo {
  id: string;
  id_full: string;
  name: string | null;
  self: boolean;
  online: boolean;
  path: PathKind | null; // null — no path, or it is ourselves
  rtt_ms: number | null;
  exit: boolean | null; // null — unknown
  country: string | null; // not described in the §1.3 prose, present in the code's `PeerInfo`
  version: string | null; // version of the member's program; null — not announced
  checked: number;
  stale: boolean;
  last_seen_sec: number;
}

export interface DomainInfo {
  name: string;
  /** The key is the member's short id; `null` — there is no record from them. */
  checks: Record<string, CheckInfo | null>;
  checking: boolean;
}

export interface CheckInfo {
  verdict: Verdict;
  reason: string;
  dns_ms: number;
  tcp_ms: number;
  tls_ms: number;
  http_ms: number;
  status: number; // HTTP code, 0 — it did not reach HTTP
  bytes: number;
  stalled: boolean;
  age_sec: number;
}

export interface RouteState {
  state: RouteMode;
  exits: ChosenExit[]; // the whole `route.exits` in config order
}

export interface ChosenExit {
  id: string;
  id_full: string;
  name: string | null;
  online: boolean;
  exit: boolean | null;
  country: string | null; // not described in the §1.3 prose, present in the code's `ChosenExit`
  active: boolean; // true for at most one
  removed?: boolean; // the exit is no longer in the network
}

export interface PortsInfo {
  published: OwnPort[];
  network: NetPort[];
}

/** Own publication; `local` — whether the local port answered. */
export interface OwnPort {
  port: number;
  name: string;
  local: "unknown" | "ok" | "refused"; // a String in the code, the values come from the `OwnPort` comment
}

/** Port of another node. `published: false` — opened here, and there is no valid "all" record. */
export interface NetPort {
  node: string; // full id
  node_name: string | null;
  online: boolean;
  port: number;
  name: string;
  published: boolean;
  opened: OpenedPort | null;
  web_url: string | null; // link at the member's mesh address; null while the port is not shared with this node or there is no proxy
}

export interface OpenedPort {
  local: number;
  state: "listening" | "bind_failed"; // a String in the code, the values come from the `OpenedInfo` comment
  error: string | null;
}

export interface LogSummary {
  tags: string[];
  ring_lines: number;
  first_seq: number;
  last_seq: number;
  tail: LogLine[]; // at most the last 6 lines
}

export interface LogLine {
  seq: number;
  t: number; // seconds since start, one decimal place
  tag: string;
  text: string; // without the timestamp and the tag
  net?: string; // slot id the line belongs to; absent for process-wide lines
}

// ---------------------------------------------------------------- GET /api/log

/** Log page, `GET /api/log?after=&limit=` — docs/WEB-API.md §1.4.
 * `GET /api/log/download` returns `text/plain`, not JSON (§1.5). */
export interface LogResponse {
  boot_id: string;
  first_seq: number;
  last_seq: number;
  gap: boolean; // true — part of the lines after `after` has already been pushed out of the ring
  lines: LogLine[];
}
