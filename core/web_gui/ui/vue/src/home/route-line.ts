/* The first part of the header line with two or more networks: how sites open is decided by the network that carries
 * the browser proxy, which is not always the leading one. `null` — the usual line of the leading network. */

import { proxySlotOf } from "../core/exit-via.ts";
import { isMulti, netName, stateOf } from "../core/slots.ts";
import type { Store } from "../core/store.ts";
import { chosenExitIds } from "../common/exits.ts";
import { peerLabel } from "../common/words.ts";
import type { NodeState, PeerInfo } from "../core/api-types.ts";
import { chosenOf } from "./exit-groups.ts";
import { netsViaText } from "./nets-texts.ts";

export interface RouteHead {
  level: "green" | "red";
  text: string;
}

const WONT_OPEN = "Sites won't open";
const wontOpen: RouteHead = { level: "red", text: WONT_OPEN };

// The member chosen as the exit, when it is known in the network and no longer offers an exit.
function stoppedOffering(peer: PeerInfo | undefined): boolean {
  return !!peer && peer.online && peer.exit !== true;
}

function viaHead(store: Store, slot: string, peer: PeerInfo, state: NodeState): RouteHead | null {
  if (state.route?.state === "unavailable") return null;
  if (slot === store.lead) return null;
  return { level: "green", text: netsViaText(peerLabel(peer, false, state.peers || []), netName(store.nets, slot)) };
}

function chosenHead(store: Store, slot: string): RouteHead | null {
  const state = stateOf(store, slot);
  const id = chosenOf(store, slot, chosenExitIds(store))[0];
  if (!state || id === undefined) return null;
  const peer = (state.peers || []).find((p) => p.id_full === id);
  if (stoppedOffering(peer)) return wontOpen;
  return peer ? viaHead(store, slot, peer, state) : null;
}

export function routeLine(store: Store): RouteHead | null {
  if (!isMulti(store)) return null;
  const slot = proxySlotOf(store);
  return slot === null ? wontOpen : chosenHead(store, slot);
}
