/* Join requests waiting for this admin to let them in, per network and in total. */

import { membersOf, stateOf, visibleSlots } from "../core/slots.ts";
import type { Store } from "../core/store.ts";
import { admittable, isMeExcluded, meId } from "../members/members-view.ts";
import { peersHash } from "../shell/nav.ts";

// Join requests to let in, in one network; nothing for a computer that is out of it or is not an admin there.
export function slotVotes(store: Store, slot: string): number {
  const state = stateOf(store, slot);
  const members = membersOf(store, slot);
  if (isMeExcluded(state, members) || !(members?.me_admin ?? state?.network?.me_role === "admin")) return 0;
  return admittable(members, meId(state)).length;
}

// The sum over the visible networks (the badge of the Networks tab and of the top bar).
export function totalVotes(store: Store): number {
  return visibleSlots(store.nets, store.slots).reduce((sum, slot) => sum + slotVotes(store, slot), 0);
}

// Where the "join requests" link leads: the only network with requests when there are several networks.
export function votesTarget(store: Store): string {
  const visible = visibleSlots(store.nets, store.slots);
  const waiting = visible.filter((slot) => slotVotes(store, slot) > 0);
  return visible.length >= 2 && waiting.length === 1 ? peersHash(waiting[0]) : "#peers";
}
