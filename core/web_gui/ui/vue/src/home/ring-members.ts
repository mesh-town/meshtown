/* Ring members and their roles from the snapshot: a pure function without Vue. */

import { compareLabels } from "../common/locale.ts";
import type { ChosenExit, NodeState, PeerInfo } from "../core/api-types.ts";
import { ROLE_ORDER } from "./ring-config.ts";
import type { RingRole } from "./ring-config.ts";

export interface RingMember {
  role: RingRole;
  label: string;
  idFull: string | null;
  /** The path to the member goes through another member (`path === "relay"`); only the exit we use is marked. */
  relayed: boolean;
}

type Snapshot = Partial<NodeState> | null;

const labelOf = (item: { name: string | null; id: string }): string => item.name || item.id;

// The active exit — only when `route.state === "exit"`: the one marked `active`, otherwise the first.
function activeExit(state: Partial<NodeState>): ChosenExit | null {
  const route = state.route;
  if (!route || route.state !== "exit") return null;
  const exits = route.exits || [];
  return exits.find((e) => e.active) || exits[0] || null;
}

function memberOf(peer: PeerInfo, viaId: string | null, clients: string[]): RingMember {
  const role = roleOf(peer, viaId, clients);
  return { role, label: labelOf(peer), idFull: peer.id_full, relayed: role === "via" && peer.path === "relay" };
}

function roleOf(peer: PeerInfo, viaId: string | null, clients: string[]): RingRole {
  if (!peer.online) return "offline";
  if (viaId !== null && peer.id_full === viaId) return "via";
  if (clients.includes(peer.id_full)) return "client";
  return "online";
}

const rank = (m: RingMember): number => ROLE_ORDER.indexOf(m.role);
const byRoleThenLabel = (a: RingMember, b: RingMember): number => rank(a) - rank(b) || compareLabels(a.label, b.label);

function missingVia(exit: ChosenExit | null, peers: PeerInfo[]): RingMember[] {
  if (exit === null || peers.some((p) => p.id_full === exit.id_full)) return [];
  return [{ role: "via", label: labelOf(exit), idFull: exit.id_full, relayed: false }];
}

export function ringMembers(state: Snapshot): RingMember[] {
  if (!state || !state.peers) return [];
  const peers = state.peers.filter((p) => !p.self);
  const exit = activeExit(state);
  const viaId = exit ? exit.id_full : null;
  const clients = state.exit?.clients || [];
  const found: RingMember[] = peers.map((p) => memberOf(p, viaId, clients));
  return [...found, ...missingVia(exit, peers)].sort(byRoleThenLabel);
}
