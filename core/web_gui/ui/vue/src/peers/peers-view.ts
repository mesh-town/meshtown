/* The Members pane of a network tab: pure logic of the list. A row is a dot, the name and
 * id, the connection, the action. */

import { agoText } from "../common/words.ts";
import {
  PEERS_DIRECT, PEERS_OFFLINE, PEERS_ONLINE, PEERS_RELAY, PEERS_SELF_EXIT_OFF, PEERS_SELF_EXIT_ON,
  peersOfflineSeenText, peersOlderTitleText, peersRttText,
} from "./peers-texts.ts";
import type { NodeState, PeerInfo } from "../core/api-types.ts";

type PeerActionKind = "self" | "chosen-ok" | "chosen-down" | "button" | "refused" | "none";

export function countryText(p: Pick<PeerInfo, "country">): string {
  return p.country || "—";
}

export function versionText(p: Pick<PeerInfo, "version">): string {
  return typeof p.version === "string" ? p.version : "";
}

export function countryCell(p: Pick<PeerInfo, "country" | "version">): string {
  if (!p.country && versionText(p)) return "";
  return countryText(p);
}

function versionCore(v: unknown): number[] | null {
  if (typeof v !== "string") return null;
  const core = v.split(/[-+]/)[0];
  return /^\d+\.\d+\.\d+$/.test(core) ? core.split(".").map(Number) : null;
}

function firstDiff(a: number[], b: number[]): number {
  return a.findIndex((n, i) => n !== b[i]);
}

export function isOlderVersion(peerVersion: unknown, ownVersion: unknown): boolean {
  const a = versionCore(peerVersion);
  const b = versionCore(ownVersion);
  if (!a || !b) return false;
  const i = firstDiff(a, b);
  return i >= 0 && a[i] < b[i];
}

export function olderMark(p: PeerInfo, state: NodeState): boolean {
  return !p.self && isOlderVersion(p.version, state.node.version);
}

export function olderTitle(p: PeerInfo, state: NodeState): string {
  if (!olderMark(p, state)) return "";
  return peersOlderTitleText(p.version, state.node.version);
}

export function formatSeen(sec: number | null | undefined): string {
  return agoText(sec) || "—";
}

function offlineText(p: PeerInfo): string {
  return p.last_seen_sec == null ? PEERS_OFFLINE : peersOfflineSeenText(formatSeen(p.last_seen_sec));
}

function onlineText(p: PeerInfo): string {
  const parts = [PEERS_ONLINE];
  if (p.path === "direct") parts.push(PEERS_DIRECT);
  if (p.path === "relay") parts.push(PEERS_RELAY);
  if (p.rtt_ms != null) parts.push(peersRttText(p.rtt_ms));
  return parts.join(" · ");
}

// The connection line: for "You" — whether sites can be opened through you, for the others — the state and the path.
export function peerMetaText(p: PeerInfo, state: NodeState): string {
  if (p.self) return state.exit.enabled ? PEERS_SELF_EXIT_ON : PEERS_SELF_EXIT_OFF;
  return p.online ? onlineText(p) : offlineText(p);
}

// "You" first, the rest in snapshot order.
export function orderedPeers(peers: PeerInfo[] | null | undefined): PeerInfo[] {
  const list = peers || [];
  return list.filter((p) => p.self).concat(list.filter((p) => !p.self));
}

// The row action: { kind } — self | chosen-ok | chosen-down | button | refused | none. A member blocked on this
// computer (`blocked`) cannot be made the exit: no button.
export function peerAction(p: PeerInfo, chosen: string[], blocked = false): { kind: PeerActionKind } {
  if (p.self) return { kind: "self" };
  if (chosen.includes(p.id_full)) return { kind: p.online && p.exit !== false ? "chosen-ok" : "chosen-down" };
  if (blocked) return { kind: "none" };
  if (p.exit === true) return { kind: "button" };
  return { kind: p.exit === false ? "refused" : "none" };
}
