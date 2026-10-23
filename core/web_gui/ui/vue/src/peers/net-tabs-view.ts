/* What the tab row and the tab header show, derived from the network list. */

import { netName, visibleSlots } from "../core/slots.ts";
import type { Store } from "../core/store.ts";
import type { NetBrief } from "../core/api-types.ts";
import { slotVotes } from "./net-votes.ts";
import { PEERS_NEW_NET, peersTabSub, peersTabTitle, peersWhoText } from "./peers-texts.ts";

export interface NetTab {
  slot: string;
  name: string;
  sub: string;
  title: string;
  votes: number;
}

const briefOf = (store: Store, slot: string): NetBrief | undefined => store.nets.find((n) => n.id === slot);

// No connection: the node says so, or it knows members and none of them is online.
export const isDown = (b: NetBrief): boolean => b.status === "broken" || (b.peers_total > 0 && b.peers_online === 0);

function tabOf(store: Store, slot: string): NetTab {
  const b = briefOf(store, slot);
  const name = netName(store.nets, slot);
  const down = b ? isDown(b) : false;
  const online = b ? b.peers_online : 0;
  const total = b ? b.peers_total : 0;
  return { slot, name, sub: peersTabSub(online, down), title: peersTabTitle(name, online, total, down), votes: slotVotes(store, slot) };
}

export const netTabs = (store: Store): NetTab[] => visibleSlots(store.nets, store.slots).map((slot) => tabOf(store, slot));

// Networks being joined: no key yet, so they have no tab of their own.
export const joiningTabs = (store: Store): { name: string }[] =>
  store.nets.filter((n) => n.joining && n.id_hex == null).map((n) => ({ name: n.label || PEERS_NEW_NET }));

export function netWho(store: Store, slot: string): string {
  const b = briefOf(store, slot);
  return peersWhoText(b ? b.peers_online : 0, b ? b.peers_total : 0);
}
