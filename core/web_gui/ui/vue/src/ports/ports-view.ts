/* Pure logic of the "Shared services (ports)" section (`ports-view.ts`): the lists `ports.published` and `ports.opened`,
 * the row status text, the key of the "Network ports" row. */

import type { NetPort, OwnPort, PeerInfo, PortProto } from "../core/api-types.ts";
import { netConfig, stateOf, visibleSlots } from "../core/slots.ts";
import type { Store } from "../core/store.ts";
import type { ConfigDocument, OpenedPortConfig, PublishedPortConfig } from "../core/api-types.ts";
import {
  PORTS_ADD_ERROR_NO_NODES, PORTS_ADD_ERROR_RANGE, PORTS_AUDIENCE_ALL, PORTS_AUDIENCE_NOBODY, PORTS_EMPTY_NET, PORTS_EMPTY_OWN,
  PORTS_OFFLINE, PORTS_ONLINE, PORTS_STATUS_OK, PORTS_STATUS_REFUSED, PORTS_STATUS_UNKNOWN, portsAlreadyPublishedText,
  PORTS_STATUS_UDP, portsBindFailedText, portsNodeLabelText, portsNotPublishedText, portsOpenErrorText, portsLocalOwnerText, portsLocalUsedText,
} from "./ports-texts.ts";
import { portKey, protoOf } from "./ports-proto.ts";
export { lastErrorText, portKey, protoOf } from "./ports-proto.ts";

export type PortsConfigKey = "published" | "opened";
export interface PortsConfigLists {
  published: PublishedPortConfig[];
  opened: OpenedPortConfig[];
}
type PublishedAfterAdd = { list: PublishedPortConfig[]; error?: undefined } | { error: string; list?: undefined };

export type AudienceMode = "all" | "nodes";
export interface AudienceCandidate {
  idFull: string;
  label: string;
  short: string;
  online: boolean;
}

const PORT_MIN = 1;
const PORT_MAX = 65535;

const ADD_ERROR_RANGE = PORTS_ADD_ERROR_RANGE;
export const ADD_ERROR_NO_NODES = PORTS_ADD_ERROR_NO_NODES;
const SHORT_ID_CHARS = 10;
export const EMPTY_OWN = PORTS_EMPTY_OWN;
export const EMPTY_NET = PORTS_EMPTY_NET;

const OWN_STATUS_TEXT: Record<string, string> = { ok: PORTS_STATUS_OK, refused: PORTS_STATUS_REFUSED, unknown: PORTS_STATUS_UNKNOWN };

const sameEntry = (port: number, proto: PortProto) => (p: { port: number; proto?: PortProto }) => p.port === port && protoOf(p) === proto;

function inRange(port: number): boolean {
  return Number.isInteger(port) && port >= PORT_MIN && port <= PORT_MAX;
}

// Success — `{ list }`, refusal — `{ error: "text" }`. The source list is not changed.
// `audience` — `"all"` or full node ids; an empty array is an error.
// A TCP entry carries no `proto` field (as the node returns it), a UDP one carries `proto: "udp"`.
export function publishedAfterAdd(list: PublishedPortConfig[], port: number, name: string, audience: "all" | string[] = "all", proto: PortProto = "tcp"): PublishedAfterAdd {
  if (!inRange(port)) return { error: ADD_ERROR_RANGE };
  if (list.some(sameEntry(port, proto))) return { error: portsAlreadyPublishedText(port, proto) };
  if (Array.isArray(audience) && audience.length === 0) return { error: ADD_ERROR_NO_NODES };
  return { list: list.concat([proto === "udp" ? { port, proto, name, for: audience } : { port, name, for: audience }]) };
}

// The first 10 characters of the full id.
export function shortId(idFull: string): string {
  return idFull.slice(0, SHORT_ID_CHARS);
}

// Candidate recipients: all members except us, offline ones too, in the `peers` order.
export function audienceCandidates(peers: PeerInfo[] | null | undefined): AudienceCandidate[] {
  return (peers || [])
    .filter((p) => !p.self)
    .map((p) => ({ idFull: p.id_full, label: p.name && p.name.trim() ? p.name : shortId(p.id_full), short: shortId(p.id_full), online: p.online }));
}

// `"all"` or the checked ids that are still among the candidates, in the candidates' order.
export function audienceOf(mode: AudienceMode, selected: string[], candidates: AudienceCandidate[]): "all" | string[] {
  if (mode === "all") return "all";
  return candidates.filter((c) => selected.includes(c.idFull)).map((c) => c.idFull);
}

// Editor candidates: the ids from `for` that are not among the members now are added to the list
// (the node left the network) — otherwise `audienceOf` would silently drop them on save.
export function audienceCandidatesWith(peers: PeerInfo[] | null | undefined, ids: string[]): AudienceCandidate[] {
  const base = audienceCandidates(peers);
  const missing = ids.filter((id) => !base.some((c) => c.idFull === id));
  return base.concat(missing.map((id) => ({ idFull: id, label: shortId(id), short: shortId(id), online: false })));
}

function peerNameOrShortId(idFull: string, peers: PeerInfo[] | null | undefined): string {
  const peer = (peers || []).find((p) => p.id_full === idFull);
  return peer && peer.name && peer.name.trim() ? peer.name : shortId(idFull);
}

// The "Shared with" column in "My published ports".
export function audienceText(entry: Pick<PublishedPortConfig, "for"> | undefined, peers: PeerInfo[] | null | undefined): string {
  if (!entry) return "—";
  if (entry.for === "all") return PORTS_AUDIENCE_ALL;
  if (entry.for.length === 0) return PORTS_AUDIENCE_NOBODY;
  return entry.for.map((id) => peerNameOrShortId(id, peers)).join(", ");
}

// Editing the name and "shared with" of an already published entry; the port number is the key and does not change.
// The source list is not changed, the order of entries stays.
export function publishedAfterEdit(list: PublishedPortConfig[], port: number, name: string, audience: "all" | string[], proto: PortProto = "tcp"): PublishedAfterAdd {
  if (!list.some(sameEntry(port, proto))) return { error: portsNotPublishedText(port, proto) };
  if (Array.isArray(audience) && audience.length === 0) return { error: ADD_ERROR_NO_NODES };
  return { list: list.map((p) => (sameEntry(port, proto)(p) ? { ...p, name, for: audience } : p)) };
}

export function publishedAfterRemove(list: PublishedPortConfig[], port: number, proto: PortProto = "tcp"): PublishedPortConfig[] {
  return list.filter((p) => !sameEntry(port, proto)(p));
}

// The shift of the default local number: the member's port 80 opens on our side as 2080 (ports below 1024
// cannot be taken by an ordinary user).
export const LOCAL_PORT_SHIFT = 2000;

const LOCAL_MIN = 1024;

// The member's port plus the shift; if it does not fit in 65535 — the member's own number. A taken number moves up
// to the next free one, then wraps to 1024; nothing free — the first candidate (the node refuses it).
export function defaultLocalPort(port: number, taken: ReadonlySet<number> = new Set()): number {
  const first = port + LOCAL_PORT_SHIFT <= PORT_MAX ? port + LOCAL_PORT_SHIFT : port;
  if (!taken.has(first)) return first;
  for (let n = first + 1; n <= PORT_MAX; n++) if (!taken.has(n)) return n;
  for (let n = LOCAL_MIN; n < first; n++) if (!taken.has(n)) return n;
  return first;
}

// Empty or not an integer — the default number.
function localPortOf(local: unknown, port: number, taken?: ReadonlySet<number>): number {
  const n = Number(local);
  return local === undefined || local === null || local === "" || !Number.isInteger(n) ? defaultLocalPort(port, taken) : n;
}

// The triple `(node, proto, port)` already exists — its `local` is replaced (open on another number after a failure),
// the order and the other entries stay; no such pair — the entry is appended to the end.
export function openedAfterOpen(list: OpenedPortConfig[], node: string, port: number, local?: unknown, taken?: ReadonlySet<number>, proto: PortProto = "tcp"): OpenedPortConfig[] {
  const next = localPortOf(local, port, taken);
  const same = sameOpened(node, port, proto);
  if (list.some(same)) return list.map((o) => (same(o) ? { ...o, local: next } : o));
  return list.concat([proto === "udp" ? { node, port, proto, local: next } : { node, port, local: next }]);
}

const sameOpened = (node: string, port: number, proto: PortProto) => (o: OpenedPortConfig) => o.node === node && sameEntry(port, proto)(o);

export function openedAfterClose(list: OpenedPortConfig[], node: string, port: number, proto: PortProto = "tcp"): OpenedPortConfig[] {
  return list.filter((o) => !sameOpened(node, port, proto)(o));
}

// An own row (`OwnPort`) carries `local`; someone else's (`NetPort`) — `online` and `opened`.
export function portStatusText(row: Pick<OwnPort, "local" | "proto"> | NetPort): string {
  if ("opened" in row && row.opened && row.opened.state === "bind_failed") return portsBindFailedText(row.opened.local);
  if ("proto" in row && row.proto === "udp") return PORTS_STATUS_UDP;
  if ("local" in row && row.local !== undefined) return OWN_STATUS_TEXT[row.local] || OWN_STATUS_TEXT.unknown;
  return "online" in row && row.online === false ? PORTS_OFFLINE : PORTS_ONLINE;
}

// The error line of opening a port under the button.
export function openErrorText(error: string): string {
  return portsOpenErrorText(error);
}

// The list of the config's `ports` section; no config or section — empty.
export function configPorts<K extends PortsConfigKey>(config: Pick<ConfigDocument, "ports"> | null | undefined, key: K): PortsConfigLists[K] {
  const section = config && config.ports;
  return (section && section[key]) || [];
}

// With several networks the key starts with the slot.
export function netRowKey(r: { node: string; port: number; proto?: PortProto }, slot?: string): string {
  return (slot ? slot + "/" : "") + r.node + "/" + portKey(protoOf(r), r.port);
}

// The node label in "Network ports"; without a name — "member <id>".
export function nodeLabel(r: Pick<NetPort, "node" | "node_name">): string {
  return r.node_name && r.node_name.trim() ? r.node_name : portsNodeLabelText(r.node);
}

// Ports that browsers refuse to connect to (the Fetch Standard "bad ports" list).
const BROWSER_BLOCKED_PORTS: ReadonlySet<number> = new Set([
  1, 7, 9, 11, 13, 15, 17, 19, 20, 21, 22, 23, 25, 37, 42, 43, 53, 69, 77, 79, 87, 95, 101, 102, 103, 104, 109, 110, 111, 113, 115, 117, 119,
  123, 135, 137, 139, 143, 161, 179, 389, 427, 465, 512, 513, 514, 515, 526, 530, 531, 532, 540, 548, 554, 556, 563, 587, 601, 636, 989, 990,
  993, 995, 1719, 1720, 1723, 2049, 3659, 4045, 4190, 5060, 5061, 6000, 6566, 6665, 6666, 6667, 6668, 6669, 6679, 6697, 10080,
]);

export function isBrowserBlockedPort(port: number): boolean {
  return BROWSER_BLOCKED_PORTS.has(port);
}

// Members (full ids) that share a displayed name with another member: their addresses end with a part of the id.
export function sameNameNodes(rows: Pick<NetPort, "node" | "node_name">[]): Set<string> {
  const byName = new Map<string, Set<string>>();
  for (const r of rows) byName.set(nodeLabel(r), (byName.get(nodeLabel(r)) ?? new Set()).add(r.node));
  const same = Array.from(byName.values()).filter((nodes) => nodes.size > 1);
  return new Set(same.flatMap((nodes) => Array.from(nodes)));
}

// ---- Per network ----

interface PortRef { slot: string; node: string; port: number; proto?: PortProto }
export interface NetPortItem { slot: string; row: NetPort }

/** The ports this device publishes in one network, as the node reports them. */
export function ownRows(store: Store, slot: string): OwnPort[] {
  return stateOf(store, slot)?.ports?.published ?? [];
}

/** The members' ports of one network. */
export function netPortItems(store: Store, slot: string): NetPortItem[] {
  return (stateOf(store, slot)?.ports?.network ?? []).map((row) => ({ slot, row }));
}

// Local numbers of opened ports over all visible networks, with the pair that holds each.
// Only numbers of the given protocol count: TCP 2053 and UDP 2053 do not clash.
export function takenLocals(store: Store, except?: PortRef, proto: PortProto = "tcp"): Map<number, PortRef> {
  const taken = new Map<number, PortRef>();
  for (const slot of visibleSlots(store.nets, store.slots)) {
    for (const o of configPorts(netConfig(store, slot), "opened").filter((x) => protoOf(x) === proto)) {
      if (except && except.slot === slot && except.node === o.node && except.port === o.port) continue;
      taken.set(o.local, { slot, node: o.node, port: o.port });
    }
  }
  return taken;
}

export function localUsedText(local: number, by: { peerLabel: string; net: string | null }): string {
  return portsLocalUsedText(local, portsLocalOwnerText(by.peerLabel, by.net));
}
