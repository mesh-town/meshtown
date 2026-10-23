/* Diagnostics status lights: the pure function `diagnosticChecks(state)` over the `/api/state` snapshot and
 * the "About" formatters (port of `formatUptimeText`, `formatMib` from the classic page). No Vue. */

import { NET_NAME, ROUTE_DIRECT_TEXT, peerLabel, quotedNet, routeViaText } from "../common/words.ts";
import { isMulti, netName, slotState } from "../core/slots.ts";
import { targetSlots } from "../core/save-config.ts";
import type { Store } from "../core/store.ts";
import type { NetBrief, NodeRuntime, NetworkInfo, NodeState, PeerInfo, RouteState, SocksInfo, TransportInfo, ChosenExit } from "../core/api-types.ts";
import {
  DIAG_DIRECTORY_LABEL, DIAG_DIRECTORY_OK, DIAG_DIRECTORY_WARN, DIAG_EXIT_DEFAULT_NAME, DIAG_EXIT_LABEL, DIAG_EXIT_VIA_DEFAULT,
  DIAG_NODE_RUNNING, DIAG_NODE_STARTING, DIAG_NODE_STOPPING, DIAG_NO_DATA, DIAG_NO_NETWORK, DIAG_PEERS_ALONE, DIAG_PEERS_LABEL,
  DIAG_PEERS_NONE_ONLINE, DIAG_PROXY_LABEL, DIAG_PROXY_OFF, diagExitUnavailableText, diagMibText, diagNetworkJoinedText, DIAG_NETWORK_JOINED_NO_LABEL,
  diagNetAloneText, diagNetDirectoryWarnText, diagNetNoConnectionText, diagNetPeersText, diagPeersOnlineText, diagProxyOnText, diagRouteWontOpenText, diagRunningMinutesText, diagRunningUptimeText, diagUptimeText,
} from "./diag-texts.ts";

export type CheckLevel = "ok" | "warn" | "bad" | "none";
export interface CheckRow {
  key: string;
  level: CheckLevel;
  text: string;
  href?: string;
}
/** Snapshot for the status lights: all blocks are optional (before the first node response — `null`/empty). */
export type DiagState = Partial<NodeState> | null | undefined;

export const LEVEL_ICON: Record<CheckLevel, string> = { ok: "✓", warn: "⚠", bad: "✗", none: "—" };

export function formatUptime(sec: number | null | undefined): string {
  const total = Math.floor(sec || 0);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  return diagUptimeText(h, m);
}

export function formatMib(kib: number | null | undefined): string {
  if (kib == null) return "—";
  return diagMibText((kib / 1024).toFixed(1));
}

function row(key: string, level: CheckLevel, text: string, href?: string): CheckRow {
  return href ? { key, level, text, href } : { key, level, text };
}

const noData = (key: string, what: string): CheckRow => row(key, "none", what + DIAG_NO_DATA);

// Without an uptime — just "Mesh Town is running" (the name is `NET_NAME`); under an hour — minutes only.
function runningText(sec: unknown): string {
  if (typeof sec !== "number" || !Number.isFinite(sec)) return DIAG_NODE_RUNNING;
  if (sec < 3600) return diagRunningMinutesText(Math.floor(sec / 60));
  return diagRunningUptimeText(formatUptime(sec));
}

function nodeRow(node: NodeRuntime | undefined): CheckRow {
  if (!node) return noData("node", NET_NAME);
  const { status } = node;
  if (status === "online" || status === "offline") {
    return row("node", "ok", runningText(node.uptime_sec));
  }
  if (status === "starting") return row("node", "warn", DIAG_NODE_STARTING);
  if (status === "stopping") return row("node", "warn", DIAG_NODE_STOPPING);
  return noData("node", NET_NAME);
}

function networkRow(network: NetworkInfo | null | undefined): CheckRow {
  if (!network) return row("network", "bad", DIAG_NO_NETWORK);
  const named = quotedNet(network.label).trim();
  return row("network", "ok", named ? diagNetworkJoinedText(named) : DIAG_NETWORK_JOINED_NO_LABEL);
}

function directoryRow(transport: TransportInfo | undefined): CheckRow {
  if (!transport) return noData("directory", DIAG_DIRECTORY_LABEL);
  const read = transport.record && transport.record.read_sec != null;
  if ((transport.dht && transport.dht.joined) || read) {
    return row("directory", "ok", DIAG_DIRECTORY_OK);
  }
  return row("directory", "warn", DIAG_DIRECTORY_WARN);
}

function peersRow(peers: PeerInfo[] | undefined): CheckRow {
  if (!Array.isArray(peers)) return noData("peers", DIAG_PEERS_LABEL);
  const others = peers.filter((p) => !p.self);
  const online = others.filter((p) => p.online).length;
  if (others.length === 0) {
    return row("peers", "warn", DIAG_PEERS_ALONE);
  }
  if (online === 0) return row("peers", "bad", DIAG_PEERS_NONE_ONLINE);
  const level = online < others.length ? "warn" : "ok";
  return row("peers", level, diagPeersOnlineText(online, others.length), "#peers");
}

function unavailableText(exits: ChosenExit[], peers: PeerInfo[]): string {
  const name = exits.length ? peerLabel(exits[0], false, peers) : DIAG_EXIT_DEFAULT_NAME;
  return diagExitUnavailableText(name);
}

function exitRow(route: RouteState | undefined, peers: PeerInfo[]): CheckRow {
  if (!route) return noData("exit", DIAG_EXIT_LABEL);
  const exits = route.exits || [];
  if (route.state === "direct") {
    return row("exit", "ok", ROUTE_DIRECT_TEXT);
  }
  if (route.state === "unavailable") return row("exit", "bad", unavailableText(exits, peers));
  if (route.state === "exit") {
    const active = exits.find((e) => e.active);
    const via = active ? peerLabel(active, false, peers) : DIAG_EXIT_VIA_DEFAULT;
    return row("exit", "ok", routeViaText(via));
  }
  return noData("exit", DIAG_EXIT_LABEL);
}

function proxyRow(socks: SocksInfo | undefined): CheckRow {
  if (!socks) return noData("proxy", DIAG_PROXY_LABEL);
  if (socks.enabled) return row("proxy", "ok", diagProxyOnText(socks.listen));
  return row("proxy", "warn", DIAG_PROXY_OFF);
}

export function diagnosticChecks(state: DiagState): CheckRow[] {
  const st: Partial<NodeState> = state || {};
  const head = [nodeRow(st.node), networkRow(st.network)];
  if (!st.network) return [...head, proxyRow(st.socks)];
  return [
    ...head,
    directoryRow(st.transport),
    peersRow(st.peers),
    exitRow(st.route, st.peers || []),
    proxyRow(st.socks),
  ];
}

// --- Several networks: process rows once, then rows per network. ---

function netPeersRow(slot: string, name: string, peers: PeerInfo[] | undefined): CheckRow {
  const key = "peers:" + slot;
  const others = (peers || []).filter((p) => !p.self);
  const online = others.filter((p) => p.online).length;
  if (others.length === 0) return row(key, "warn", diagNetAloneText(name));
  if (online === 0) return row(key, "bad", diagNetNoConnectionText(name));
  return row(key, online < others.length ? "warn" : "ok", diagNetPeersText(name, online, others.length), "#peers/" + slot);
}

/** The rows of one network: its members and, when it is not connected, its address directory. */
export function netRows(brief: NetBrief, state: NodeState | null, name: string = brief.label ?? brief.id): CheckRow[] {
  if (!state) return [row("peers:" + brief.id, "bad", diagNetNoConnectionText(name))];
  const rows = [netPeersRow(brief.id, name, state.peers)];
  if (directoryRow(state.transport).level !== "warn") return rows;
  return [...rows, row("directory:" + brief.id, "warn", diagNetDirectoryWarnText(name))];
}

function multiExitRow(store: Store, slots: string[]): CheckRow {
  const slot = slots.find((id) => store.nets.find((n) => n.id === id)?.proxy);
  const state = slot ? slotState(store, slot) : null;
  const route = state?.route;
  if (!slot || !route || route.state === "direct") return row("exit", "ok", ROUTE_DIRECT_TEXT);
  const peers = state.peers || [];
  const exits = route.exits || [];
  if (route.state === "unavailable") return row("exit", "bad", diagRouteWontOpenText(exits.length ? peerLabel(exits[0], false, peers) : DIAG_EXIT_DEFAULT_NAME));
  const active = exits.find((e) => e.active);
  const via = (active ? peerLabel(active, false, peers) : DIAG_EXIT_VIA_DEFAULT) + " (" + netName(store.nets, slot) + ")";
  return row("exit", "ok", routeViaText(via));
}

/** The status rows of the page: with one network exactly `diagnosticChecks(state)`. */
export function diagnosticRows(store: Store): CheckRow[] {
  if (!isMulti(store)) return diagnosticChecks(store.state);
  const slots = targetSlots(store);
  const main = store.slots.main?.state ?? store.state;
  const perNet = slots.flatMap((id) => {
    const brief = store.nets.find((n) => n.id === id) as NetBrief;
    return netRows(brief, slotState(store, id), netName(store.nets, id));
  });
  return [nodeRow(main?.node), ...perNet, multiExitRow(store, slots), proxyRow(main?.socks)];
}
