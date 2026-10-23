/* The exit lists of "Exit via…": one group per network the node is in. */

import { membersOf, netName, stateOf } from "../core/slots.ts";
import { targetSlots } from "../core/save-config.ts";
import { proxySlotOf } from "../core/exit-via.ts";
import type { Store } from "../core/store.ts";
import { getRememberedFor } from "../common/exits.ts";
import { memberExits } from "../members/exit-members.ts";
import { exitCandidates } from "./exit-list.ts";
import type { ExitCandidate } from "./exit-list.ts";

/** The exit candidates of one network; the title is drawn when several networks are shown. */
export interface ExitGroup {
  slot: string;
  title: string;
  items: ExitCandidate[];
}

type Remembered = (slot: string) => string[];

// The chosen exits of a slot: only the proxy slot has any; its leading form comes from the page config.
function chosenOf(store: Store, slot: string, lead: string[]): string[] {
  if (proxySlotOf(store) !== slot) return [];
  if (slot === store.lead) return lead;
  return (stateOf(store, slot)?.route?.exits || []).map((e) => e.id_full);
}

export function exitGroups(store: Store, lead: string[], remembered: Remembered): ExitGroup[] {
  return targetSlots(store).map((slot) => ({
    slot,
    title: netName(store.nets, slot),
    items: memberExits(exitCandidates(stateOf(store, slot), chosenOf(store, slot, lead), remembered(slot)), membersOf(store, slot)),
  }));
}

export const rememberedIn = (storage: Parameters<typeof getRememberedFor>[1]): Remembered => (slot) => getRememberedFor(slot, storage);
