/* The ring of one network tile on Home: a pure function of the store, no Vue. */

import type { NodeState } from "../core/api-types.ts";
import { proxySlotOf } from "../core/exit-via.ts";
import type { Store } from "../core/store.ts";
import { ringMembers } from "./ring-members.ts";
import type { RingMember } from "./ring-members.ts";
import { ringCenter } from "./ring-text.ts";
import type { RingCenter } from "./ring-text.ts";

export interface NetRing {
  members: RingMember[];
  center: RingCenter;
}

// The exit lives only in the proxy slot: other slots' routes must not put it in their rings.
function stateForRing(store: Store, slot: string): Partial<NodeState> | null {
  const state = store.slots[slot]?.state ?? null;
  if (state === null || slot === proxySlotOf(store)) return state;
  const { route: _route, ...rest } = state;
  return rest;
}

export function netRing(store: Store, slot: string): NetRing {
  const known = store.slots[slot]?.state != null;
  const members = ringMembers(stateForRing(store, slot));
  return { members, center: ringCenter(members, known ? "green" : "grey") };
}
