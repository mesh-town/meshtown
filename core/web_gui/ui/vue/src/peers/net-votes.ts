/* Requests waiting for this member's confirmation, per network and in total. */

import { membersOf, stateOf, visibleSlots } from "../core/slots.ts";
import type { Store } from "../core/store.ts";
import { admittable, confirmable, isMeExcluded, meId } from "../members/members-view.ts";
import { peersHash } from "../shell/nav.ts";

// Removals plus join requests to confirm in one network; nothing for a computer that is out of it.
export function slotVotes(store: Store, slot: string): number {
  const state = stateOf(store, slot);
  const members = membersOf(store, slot);
  if (isMeExcluded(state, members)) return 0;
  const me = meId(state);
  return confirmable(members, me).length + admittable(members, me).length;
}

// The sum over the visible networks (the badge of the Members tab and of the top bar).
export function totalVotes(store: Store): number {
  return visibleSlots(store.nets, store.slots).reduce((sum, slot) => sum + slotVotes(store, slot), 0);
}

// Where the "requests to confirm" link leads: the only network with requests when there are several networks.
export function votesTarget(store: Store): string {
  const visible = visibleSlots(store.nets, store.slots);
  const waiting = visible.filter((slot) => slotVotes(store, slot) > 0);
  return visible.length >= 2 && waiting.length === 1 ? peersHash(waiting[0]) : "#peers";
}
