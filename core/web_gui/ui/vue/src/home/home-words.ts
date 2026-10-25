/* Home words — pure functions: the member counter, the "suddenly alone" network status, the hint
 * for the "Allow others…" switch. */

import { forDevice } from "../common/device.ts";
import { plural } from "../common/locale.ts";
import type { NodeState, PeerInfo } from "../core/api-types.ts";
import { ALONE_INVITE_TEXT, ALONE_TEXT, NET_LOADING_TEXT, NO_CATALOG_INVITE_TEXT, noCatalogOthersText, offlineOthersText, quotedNet } from "../common/words.ts";

function allOnline(total: number): string {
  return total === 1 ? "online" : total === 2 ? "both online" : "all online";
}

// "both online" / "2 online" / "none online" — one word "online" across the whole program.
export function onlineCountText(total: number, online: number): string {
  if (online === total) return allOnline(total);
  return online === 0 ? "none online" : online + " online";
}

// 'Network "X" · ' — only for a snapshot with a network label; without a label the line is as before.
type PeersSource = Partial<NodeState> | Pick<PeerInfo, "online" | "self">[];

function netLabelOf(source: PeersSource): string | null {
  return !Array.isArray(source) && source.network ? source.network.label : "";
}

function netPrefix(source: PeersSource): string {
  const label = netLabelOf(source);
  return quotedNet(label) ? "Network" + quotedNet(label) + " · " : "";
}

// `peers` — an array of member rows or a whole snapshot; no network (no `network`) — null.
// The source of truth is peers[], not live membership: it can drop to 1.
export function membersCountText(source: PeersSource | null | undefined): string | null {
  if (!Array.isArray(source) && !(source && source.network)) return null;
  return countText(source, (Array.isArray(source) ? source : source.peers) || []);
}

// One person — "only you for now"; from two — "you and N others, …" (the online count is over the others).
function countText(source: PeersSource, peers: Pick<PeerInfo, "online" | "self">[]): string {
  if (peers.length <= 1) return alonePrefix(source);
  const others = peers.filter((p) => !p.self);
  const online = others.filter((p) => p.online).length;
  const noun = plural(others.length, "other", "others");
  return netPrefix(source) + "you and " + others.length + " " + noun + ", " + onlineCountText(others.length, online);
}

function alonePrefix(source: PeersSource): string {
  const label = netLabelOf(source);
  const net = quotedNet(label);
  return net ? "Only you in" + net + " so far. Invite the people you trust." : ALONE_TEXT.charAt(0).toUpperCase() + ALONE_TEXT.slice(1);
}

export type NetPhase = "online" | "loading" | "no_catalog" | "offline" | "alone";

function others(state: Partial<NodeState> | null | undefined): PeerInfo[] {
  return ((state && state.peers) || []).filter((p) => !p.self);
}

function dhtJoined(state: Partial<NodeState> | null | undefined): boolean {
  return Boolean(state && state.transport && state.transport.dht && state.transport.dht.joined);
}

// The node itself reported that the directory is not connected: strictly `joined === false`, otherwise "unknown".
function dhtDown(state: Partial<NodeState> | null | undefined): boolean {
  return Boolean(state && state.transport && state.transport.dht && state.transport.dht.joined === false);
}

const LOADING_GRACE_SEC = 60;

// An unknown uptime also counts as recent.
function startedRecently(state: Partial<NodeState> | null | undefined): boolean {
  const up = state && state.node && state.node.uptime_sec;
  return typeof up !== "number" || up < LOADING_GRACE_SEC;
}

// Check order: online → directory not connected and the node started recently (first 60 s) 
// → directory unavailable after 60 s → others offline → nobody.
export function netPhase(state: Partial<NodeState> | null | undefined): NetPhase {
  const rest = others(state);
  if (rest.some((p) => p.online)) return "online";
  if (!dhtJoined(state) && startedRecently(state)) return "loading";
  if (dhtDown(state) && !startedRecently(state)) return "no_catalog";
  return rest.length > 0 ? "offline" : "alone";
}

const capitalize = (text: string): string => text.charAt(0).toUpperCase() + text.slice(1);

const NET_STATUS = new Map<NetPhase, (n: number) => string | null>([
  ["online", () => null],
  ["loading", () => capitalize(NET_LOADING_TEXT)],
  ["no_catalog", (n) => (n === 0 ? NO_CATALOG_INVITE_TEXT : noCatalogOthersText(n))],
  ["offline", (n) => capitalize(offlineOthersText(n)) + " — looking for them through the address directory."],
  ["alone", () => ALONE_INVITE_TEXT],
]);

// The Home network status — only when no other member is online.
export function netStatusText(state: Partial<NodeState> | null | undefined): string | null {
  if (!state || !state.network) return null;
  return (NET_STATUS.get(netPhase(state)) as (n: number) => string | null)(others(state).length);
}

// The explanation under the "Allow others…" switch.
export function exitServerHint(enabled: boolean, label: string | null): string {
  return enabled
    ? "On: members of the network" + quotedNet(label) +
      " can open sites through your internet connection, and sites will see your address."
    : forDevice("Off: no one can open sites through this computer.");
}

// Home tabs: the strip caption and the button texts.
export const HOME_TABS_LABEL = "Home sections";
export const HOME_TAB_TEXTS: ReadonlyArray<readonly ["main" | "wizard", string]> = [
  ["main", "Overview"],
  ["wizard", "Setup"],
];
