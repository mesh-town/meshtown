/* What the tab row and the tab header show, derived from the network list. */

import { NEW_NET_NAME, membersOf, netName, visibleSlots } from "../core/slots.ts";
import { isMeRemoved } from "../members/members-ids.ts";
import type { Store } from "../core/store.ts";
import type { NetBrief } from "../core/api-types.ts";
import { slotVotes } from "./net-votes.ts";
import { PEERS_TAB_DELETED_SUB, PEERS_TAB_JOINING_SUB, PEERS_TAB_REMOVED_SUB, peersManagedTabSub, peersTabSub, peersTabTitle, peersWhoText } from "./peers-texts.ts";

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

// A deleted network says so, and so does one that dropped this computer from its member list; a Managed one says "managed" before the count (not when the connection is down).
function subOf(store: Store, slot: string, b: NetBrief | undefined, online: number, down: boolean): string {
  if (b?.id_hex && store.notices.some((n) => n.id_hex === b.id_hex)) return PEERS_TAB_DELETED_SUB;
  if (isMeRemoved(membersOf(store, slot))) return PEERS_TAB_REMOVED_SUB;
  return b?.kind === "Managed" && !down ? peersManagedTabSub(online) : peersTabSub(online, down);
}

function tabOf(store: Store, slot: string): NetTab {
  const b = briefOf(store, slot);
  const name = netName(store.nets, slot);
  const down = b ? isDown(b) : false;
  const online = b ? b.peers_online : 0;
  const total = b ? b.peers_total : 0;
  const waiting = b ? b.joining && b.id_hex == null : false;
  const sub = waiting ? PEERS_TAB_JOINING_SUB : subOf(store, slot, b, online, down);
  return { slot, name, sub, title: peersTabTitle(name, online, total, down), votes: slotVotes(store, slot) };
}

export const netTabs = (store: Store): NetTab[] => visibleSlots(store.nets, store.slots).map((slot) => tabOf(store, slot));

// Networks being joined whose state has not arrived yet: they are not among the regular tabs.
export const joiningTabs = (store: Store): { name: string }[] => {
  const shown = visibleSlots(store.nets, store.slots);
  return store.nets.filter((n) => n.joining && n.id_hex == null && !shown.includes(n.id)).map((n) => ({ name: n.label || NEW_NET_NAME }));
};

export function netWho(store: Store, slot: string): string {
  const b = briefOf(store, slot);
  return peersWhoText(b ? b.peers_online : 0, b ? b.peers_total : 0);
}
