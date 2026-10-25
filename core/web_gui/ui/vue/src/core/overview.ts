/* `GET /api/overview`: tolerant parsing, the synthetic overview of an old node and writing a snapshot into the store. */

import type { NetBrief, NodeState, Notice, Overview, SlotEntry, SlotSlice } from "./api-types.ts";
import type { MembersReply } from "../members/members-types.ts";
import type { Store } from "./store.ts";
import { isMeExcluded } from "../members/members-ids.ts";
import { parseMembers } from "../members/members-api.ts";
import { isSlotId, visibleSlots } from "./slots.ts";

type Raw = Record<string, unknown>;

const NODE_STATUSES: readonly string[] = ["starting", "online", "offline", "stopping", "broken"];
const ROUTE_STATES: readonly string[] = ["direct", "exit", "unavailable", "removed"];

const isObj = (v: unknown): v is Raw => typeof v === "object" && v !== null && !Array.isArray(v);
const strOrNull = (v: unknown): string | null => (typeof v === "string" ? v : null);
const count = (v: unknown): number => (typeof v === "number" && Number.isFinite(v) && v >= 0 ? v : 0);
const oneOf = (v: unknown, list: readonly string[]): string => (typeof v === "string" && list.includes(v) ? v : "unavailable");
const strings = (v: unknown): string[] => (Array.isArray(v) ? v.filter((x): x is string => typeof x === "string") : []);

// The Managed fields of a brief; a Peer-only node sends none and the brief stays without them.
function managedBrief(raw: Raw): Pick<NetBrief, "kind" | "me_admin" | "can_invite"> {
  const out: Pick<NetBrief, "kind" | "me_admin" | "can_invite"> = {};
  if (raw.kind === "Managed" || raw.kind === "Peer") out.kind = raw.kind;
  if (typeof raw.me_admin === "boolean") out.me_admin = raw.me_admin;
  if (typeof raw.can_invite === "boolean") out.can_invite = raw.can_invite;
  return out;
}

function parseBrief(raw: unknown): NetBrief | null {
  if (!isObj(raw) || typeof raw.id !== "string" || !isSlotId(raw.id)) return null;
  return {
    id: raw.id,
    label: strOrNull(raw.label),
    id_hex: strOrNull(raw.id_hex),
    status: oneOf(raw.status, NODE_STATUSES),
    exit_enabled: raw.exit_enabled === true,
    proxy: raw.proxy === true,
    route_state: oneOf(raw.route_state, ROUTE_STATES),
    exits: strings(raw.exits),
    members_voting: count(raw.members_voting),
    joining: raw.joining === true,
    members_admitting: count(raw.members_admitting),
    me_excluded: raw.me_excluded === true,
    me_pending: raw.me_pending === true,
    peers_online: count(raw.peers_online),
    peers_total: count(raw.peers_total),
    admission_refused: raw.admission_refused === true,
    ...managedBrief(raw),
  };
}

function parseEntry(raw: unknown): SlotEntry | null {
  if (!isObj(raw) || typeof raw.slot !== "string" || !isSlotId(raw.slot)) return null;
  const state = isObj(raw.state) ? (raw.state as unknown as NodeState) : null;
  return { slot: raw.slot, state, members: parseMembers(raw.members) };
}

const nonNull = <T>(v: T | null): v is T => v !== null;

// Tolerant to extra and malformed fields; not an overview — null.
export function parseOverview(raw: unknown): Overview | null {
  if (!isObj(raw) || !Array.isArray(raw.slots)) return null;
  return {
    api: typeof raw.api === "number" ? raw.api : 0,
    nets: (Array.isArray(raw.nets) ? raw.nets : []).map(parseBrief).filter(nonNull),
    slots: raw.slots.map(parseEntry).filter(nonNull),
    restart_slots: strings(raw.restart_slots),
    ...(raw.notices === undefined ? {} : { notices: parseNotices(raw.notices) }),
  };
}

function parseNotices(raw: unknown): Notice[] {
  return Array.isArray(raw) ? raw.filter((n): n is Notice => isObj(n) && typeof n.id === "number" && n.kind === "net_deleted") : [];
}

const BROKEN: Omit<NetBrief, "id"> = {
  label: null,
  id_hex: null,
  status: "broken",
  exit_enabled: false,
  proxy: false,
  route_state: "unavailable",
  exits: [],
  members_voting: 0,
  joining: false,
  members_admitting: 0,
  me_excluded: false,
  me_pending: false,
  peers_online: 0,
  peers_total: 0,
  admission_refused: false,
};

/** The brief of a slot computed from its snapshot (the node did not list it, or it is an old node). */
export function briefFromState(id: string, state: NodeState | null, members: MembersReply | null): NetBrief {
  if (!state) return { id, ...BROKEN, proxy: id === "main" };
  const peers = (state.peers ?? []).filter((p) => !p.self);
  return {
    id,
    label: state.network?.label ?? null,
    id_hex: state.network?.id_hex ?? null,
    status: state.node?.status ?? "unavailable",
    exit_enabled: state.exit?.enabled === true,
    proxy: id === "main",
    route_state: state.route?.state ?? "unavailable",
    exits: (state.route?.exits ?? []).map((e) => e.id_full),
    members_voting: state.members?.voting ?? 0,
    joining: state.node?.invite_join?.state === "waiting",
    members_admitting: state.members?.admitting ?? 0,
    me_excluded: isMeExcluded(state, members),
    me_pending: members ? members.me_pending === true : state.members?.me_pending === true,
    peers_online: peers.filter((p) => p.online).length,
    peers_total: peers.length,
    admission_refused: state.members?.admission_refused === true,
    ...managedFromState(state),
  };
}

// A node without `/api/overview` carries the Managed facts in the snapshot's `network`.
function managedFromState(state: NodeState): Pick<NetBrief, "kind" | "me_admin" | "can_invite"> {
  const net = state.network;
  if (!net?.kind) return {};
  return { kind: net.kind, me_admin: net.me_role === "admin", ...(net.can_invite === undefined ? {} : { can_invite: net.can_invite }) };
}

/** The overview of a node that has no `/api/overview`: one slot, `main`. */
export function legacyOverview(state: NodeState, members: MembersReply | null): Overview {
  const restart = (state.restart_pending ?? []).length > 0;
  return {
    api: 0,
    nets: [briefFromState("main", state, members)],
    slots: [{ slot: "main", state, members }],
    restart_slots: restart ? ["main"] : [],
    notices: state.notices ?? [],
  };
}

function newSlice(id: string): SlotSlice {
  const brief = briefFromState(id, null, null);
  return { id, brief, state: null, members: null, config: null, configRev: null, configMeta: null, bootId: null };
}

function sliceFor(store: Store, entry: SlotEntry, nets: NetBrief[]): SlotSlice {
  const slice = store.slots[entry.slot] ?? newSlice(entry.slot);
  slice.brief = nets.find((n) => n.id === entry.slot) ?? briefFromState(entry.slot, entry.state, entry.members);
  slice.state = entry.state;
  slice.members = entry.members;
  return slice;
}

function buildSlices(store: Store, ov: Overview): Record<string, SlotSlice> {
  const next: Record<string, SlotSlice> = {};
  for (const entry of ov.slots) if (!(entry.slot in next)) next[entry.slot] = sliceFor(store, entry, ov.nets);
  return next;
}

function needsConfig(slice: SlotSlice): boolean {
  const state = slice.state;
  if (!state) return false;
  return slice.bootId === null || state.boot_id !== slice.bootId || state.config_rev > (slice.configRev ?? -1);
}

function configReads(nets: NetBrief[], next: Record<string, SlotSlice>): string[] {
  return visibleSlots(nets, next).filter((id) => id !== "main" && needsConfig(next[id]));
}

// The leading slot and the two values the rest of the page reads as "the node": its state and members.
// A broken leader (no state) leaves `store.state` as it was.
function applyLead(store: Store): void {
  store.lead = visibleSlots(store.nets, store.slots)[0];
  const lead = store.slots[store.lead];
  if (!lead) return;
  const main = store.slots.main?.state ?? null;
  if (lead.state) store.state = store.lead === "main" || !main ? lead.state : { ...lead.state, socks: main.socks, web: main.web, log: main.log };
  store.members = lead.members;
}

/** Writes an overview into the store in one synchronous pass and returns the slots whose config must be (re)read. */
export function acceptOverview(store: Store, ov: Overview): string[] {
  const next = buildSlices(store, ov);
  const reads = configReads(ov.nets, next);
  store.nets = ov.nets;
  store.slots = next;
  store.restartSlots = ov.restart_slots.filter((id) => id in next);
  store.notices = ov.notices ?? [];
  applyLead(store);
  return reads;
}

/** Puts a member list into a slot's slice (and into `store.members` for the leading slot). */
export function putSlotMembers(store: Store, slot: string, members: MembersReply | null): void {
  const slice = store.slots[slot];
  if (!slice) return;
  slice.members = members;
  if (slot === store.lead) store.members = members;
}

/** The page left a network: forgets its slice and re-picks the leader and the tab. */
export function dropSlot(store: Store, slot: string, nets: NetBrief[]): void {
  if (nets.length > 0) store.nets = nets;
  if (slot === "main") putSlotMembers(store, "main", null);
  else delete store.slots[slot];
  applyLead(store);
  store.peersTab = visibleSlots(store.nets, store.slots)[0];
}
