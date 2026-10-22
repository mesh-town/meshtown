/* Pure logic of the "Ports" section (`ports-view.ts`): the lists `ports.published` and `ports.opened`,
 * the row status text, the key of the "Network ports" row. A port of the classic `10-ports.js`. */

import type { NetPort, OwnPort, PeerInfo } from "../core/api-types.ts";
import type { ConfigDocument, OpenedPortConfig, PublishedPortConfig } from "../core/api-types.ts";
import {
  PORTS_ADD_ERROR_NO_NODES, PORTS_ADD_ERROR_RANGE, PORTS_AUDIENCE_ALL, PORTS_AUDIENCE_NOBODY, PORTS_EMPTY_NET, PORTS_EMPTY_OWN,
  PORTS_OFFLINE, PORTS_ONLINE, PORTS_STATUS_OK, PORTS_STATUS_REFUSED, PORTS_STATUS_UNKNOWN, portsAlreadyPublishedText,
  portsBindFailedText, portsNodeLabelText, portsNotPublishedText, portsOpenErrorText,
} from "./ports-texts.ts";

export type PortsConfigKey = "published" | "opened";
export interface PortsConfigLists {
  published: PublishedPortConfig[];
  opened: OpenedPortConfig[];
}
export type PublishedAfterAdd = { list: PublishedPortConfig[]; error?: undefined } | { error: string; list?: undefined };

export type AudienceMode = "all" | "nodes";
export interface AudienceCandidate {
  idFull: string;
  label: string;
  short: string;
  online: boolean;
}

const PORT_MIN = 1;
const PORT_MAX = 65535;

export const ADD_ERROR_RANGE = PORTS_ADD_ERROR_RANGE;
export const ADD_ERROR_NO_NODES = PORTS_ADD_ERROR_NO_NODES;
export const SHORT_ID_CHARS = 10;
export const EMPTY_OWN = PORTS_EMPTY_OWN;
export const EMPTY_NET = PORTS_EMPTY_NET;

const OWN_STATUS_TEXT: Record<string, string> = { ok: PORTS_STATUS_OK, refused: PORTS_STATUS_REFUSED, unknown: PORTS_STATUS_UNKNOWN };

function inRange(port: number): boolean {
  return Number.isInteger(port) && port >= PORT_MIN && port <= PORT_MAX;
}

// Success — `{ list }`, refusal — `{ error: "text" }`. The source list is not changed.
// `audience` — `"all"` or full node ids; an empty array is an error.
export function publishedAfterAdd(list: PublishedPortConfig[], port: number, name: string, audience: "all" | string[] = "all"): PublishedAfterAdd {
  if (!inRange(port)) return { error: ADD_ERROR_RANGE };
  if (list.some((p) => p.port === port)) return { error: portsAlreadyPublishedText(port) };
  if (Array.isArray(audience) && audience.length === 0) return { error: ADD_ERROR_NO_NODES };
  return { list: list.concat([{ port, name, for: audience }]) };
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
export function publishedAfterEdit(list: PublishedPortConfig[], port: number, name: string, audience: "all" | string[]): PublishedAfterAdd {
  if (!list.some((p) => p.port === port)) return { error: portsNotPublishedText(port) };
  if (Array.isArray(audience) && audience.length === 0) return { error: ADD_ERROR_NO_NODES };
  return { list: list.map((p) => (p.port === port ? { ...p, name, for: audience } : p)) };
}

export function publishedAfterRemove(list: PublishedPortConfig[], port: number): PublishedPortConfig[] {
  return list.filter((p) => p.port !== port);
}

// The shift of the default local number: the member's port 80 opens on our side as 2080 (ports below 1024
// cannot be taken by an ordinary user).
export const LOCAL_PORT_SHIFT = 2000;

// The member's port plus the shift; if it does not fit in 65535 — the member's own number.
export function defaultLocalPort(port: number): number {
  return port + LOCAL_PORT_SHIFT <= PORT_MAX ? port + LOCAL_PORT_SHIFT : port;
}

// Empty or not an integer — the default number.
function localPortOf(local: unknown, port: number): number {
  const n = Number(local);
  return local === undefined || local === null || local === "" || !Number.isInteger(n) ? defaultLocalPort(port) : n;
}

// The pair `(node, port)` already exists — its `local` is replaced (open on another number after a failure),
// the order and the other entries stay; no such pair — the entry is appended to the end.
export function openedAfterOpen(list: OpenedPortConfig[], node: string, port: number, local?: unknown): OpenedPortConfig[] {
  const next = localPortOf(local, port);
  if (list.some((o) => o.node === node && o.port === port)) return list.map((o) => (o.node === node && o.port === port ? { ...o, local: next } : o));
  return list.concat([{ node, port, local: next }]);
}

export function openedAfterClose(list: OpenedPortConfig[], node: string, port: number): OpenedPortConfig[] {
  return list.filter((o) => !(o.node === node && o.port === port));
}

// An own row (`OwnPort`) carries `local`; someone else's (`NetPort`) — `online` and `opened`.
export function portStatusText(row: OwnPort | NetPort): string {
  if ("opened" in row && row.opened && row.opened.state === "bind_failed") return portsBindFailedText(row.opened.local);
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

export function netRowKey(r: Pick<NetPort, "node" | "port">): string {
  return r.node + "/" + r.port;
}

// The node label in "Network ports"; without a name — "member <id>".
export function nodeLabel(r: Pick<NetPort, "node" | "node_name">): string {
  return r.node_name && r.node_name.trim() ? r.node_name : portsNodeLabelText(r.node);
}
