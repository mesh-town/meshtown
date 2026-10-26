/* The line of admins above the member list of a Managed network. */

import { membersOf, stateOf } from "../core/slots.ts";
import type { Store } from "../core/store.ts";
import { displayNames } from "../members/members-ids.ts";

const SHORT_ID = 8;

const nameFor = (names: Record<string, string>, id: string): string => names[id]?.trim() || id.slice(0, SHORT_ID);

/** `Managed network · Admins: you, anna`; `admins` and `me` are full ids, `names` maps a full id to its name. */
export function adminsLineText(admins: string[], me: string, names: Record<string, string>): string {
  const list = admins.map((id) => (id === me ? "you" : nameFor(names, id)));
  return `Managed network · Admins: ${list.join(", ")}`;
}

/** The display names of the admins of a Managed network in a slot; `undefined` before the snapshot arrives. */
export function adminNamesOf(store: Store, slot: string): string[] | undefined {
  const net = stateOf(store, slot)?.network;
  if (!net?.admins?.length) return undefined;
  const names = displayNames(membersOf(store, slot));
  return net.admins.map((id) => nameFor(names, id));
}
