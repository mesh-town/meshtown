/* Network slots: addresses, visibility and display names. Pure functions over the overview data. */

import type { ConfigDocument, NetBrief, NodeState, SlotSlice } from "./api-types.ts";
import type { MembersReply } from "../members/members-types.ts";

/** The part of the page store these helpers read. */
export interface SlotsHost {
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

export function isMulti(store: SlotsHost): boolean {
  return visibleSlots(store.nets, store.slots).length >= 2;
}

const normalized = (s: string) => s.toLowerCase().replace(/\s+/g, "");

function baseName(n: NetBrief): string {
  if (n.label != null) return n.label;
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

export function slotMembers(store: SlotsHost, id: string): MembersReply | null {
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
