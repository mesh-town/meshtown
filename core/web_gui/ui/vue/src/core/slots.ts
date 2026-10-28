/* Network slots: addresses, visibility and display names. Pure functions over the overview data. */

import type { ConfigDocument, NetBrief, NodeState, SlotSlice } from "./api-types.ts";
import type { MembersReply } from "../members/members-types.ts";

/** The part of the page store these helpers read. */
interface SlotsHost {
  nets: NetBrief[];
  slots: Record<string, SlotSlice>;
  config: ConfigDocument | null;
}

const SLOT_ID = /^[a-z2-7]{6}$/;

export function slotPath(slot: string, tail: string): string {
  return slot === "main" ? `/api/${tail}` : `/api/net/${slot}/${tail}`;
}

export function isSlotId(s: string): boolean {
  return s === "main" || SLOT_ID.test(s);
}

const holdsNetwork = (n: NetBrief) => n.id_hex != null || n.joining;

// A slot is visible when it holds a key (or is being joined) and its state has arrived. No visible slots —
// the setup screen shows the main one.
export function visibleSlots(nets: NetBrief[], slots: Record<string, SlotSlice>): string[] {
  const ids = nets.filter((n) => holdsNetwork(n) && slots[n.id]?.state != null).map((n) => n.id);
  return ids.length > 0 ? ids : ["main"];
}

/** The server says the members of the slot turned this computer down. */
export const refusedFlag = (store: { nets: NetBrief[] }, id: string): boolean =>
  store.nets.find((n) => n.id === id)?.admission_refused === true;

/** A slot whose join the members refused: it holds no key, the node keeps the refusal until the user goes back. */
export const isRefused = (nets: NetBrief[], slots: Record<string, SlotSlice>, id: string): boolean =>
  nets.some((n) => n.id === id && n.id_hex == null && !n.joining) && slots[id]?.state?.node?.invite_join?.state === "refused";

/** Slots the node could not start (saved settings unreadable): no state, listed as broken. `main` is not among them. */
export function brokenSlots(nets: NetBrief[]): string[] {
  return [...new Set(nets.filter((n) => n.status === "broken" && n.id !== "main").map((n) => n.id))];
}

// The slots the Members page lists as tabs: the visible ones, those with a refused join (until the user goes back) and the broken ones.
export function peersSlots(nets: NetBrief[], slots: Record<string, SlotSlice>): string[] {
  const shown = visibleSlots(nets, slots).filter((id) => id !== "main" || nets.some((n) => n.id === "main" && holdsNetwork(n)));
  const refused = nets.map((n) => n.id).filter((id) => !shown.includes(id) && isRefused(nets, slots, id));
  const broken = brokenSlots(nets).filter((id) => !shown.includes(id) && !refused.includes(id));
  const all = [...shown, ...refused, ...broken];
  return all.length > 0 ? all : ["main"];
}

/** The tabs that stand for a running network: a broken slot (a `--key` without a key) is a tab but not a network. */
export const networkSlots = (nets: NetBrief[], ids: string[]): string[] => {
  const broken = brokenSlots(nets);
  return ids.filter((id) => !broken.includes(id));
};

export function isMulti(store: SlotsHost): boolean {
  return visibleSlots(store.nets, store.slots).length >= 2;
}

const normalized = (s: string) => s.toLowerCase().replace(/\s+/g, "");

/** The name of a slot being joined that holds no key yet. */
export const NEW_NET_NAME = "New network";

function baseName(n: NetBrief): string {
  if (n.label != null) return n.label;
  if (n.joining && n.id_hex == null) return NEW_NET_NAME;
  return n.id_hex != null ? `Network ${n.id_hex.slice(0, 4)}` : n.id;
}

function clashes(nets: NetBrief[], n: NetBrief): boolean {
  const mine = normalized(baseName(n));
  return nets.some((o) => o.id !== n.id && holdsNetwork(o) && normalized(baseName(o)) === mine);
}

export function netName(nets: NetBrief[], id: string): string {
  const n = nets.find((x) => x.id === id);
  if (!n) return id;
  const base = baseName(n);
  return n.id_hex != null && clashes(nets, n) ? `${base} · ${n.id_hex.slice(0, 4)}` : base;
}

export function slotState(store: SlotsHost, id: string): NodeState | null {
  return store.slots[id]?.state ?? null;
}

function slotMembers(store: SlotsHost, id: string): MembersReply | null {
  return store.slots[id]?.members ?? null;
}

/** The leading slot's `state`/`members` live in the store itself (they exist before any slice does); other slots in their slice. */
interface LeadHost extends SlotsHost {
  lead: string;
  state: NodeState | null;
  members: MembersReply | null;
}

export const stateOf = (store: LeadHost, id: string): NodeState | null => (id === store.lead ? store.state : slotState(store, id));

export const membersOf = (store: LeadHost, id: string): MembersReply | null => (id === store.lead ? store.members : slotMembers(store, id));

export function netConfig(store: SlotsHost, id: string): ConfigDocument | null {
  return id === "main" ? store.config : (store.slots[id]?.config ?? null);
}
