/* The plaque that replaces the invite button for a member who is not an admin of a Managed network. */

import { computed } from "vue";
import type { ComputedRef } from "vue";
import { netName, stateOf } from "../core/slots.ts";
import type { Store } from "../core/store.ts";
import { adminNamesOf } from "./admins-line.ts";

// 'a' · 'a or b' · 'a, b or c'.
function orList(names: string[]): string {
  const last = names[names.length - 1];
  return names.length < 2 ? last : `${names.slice(0, -1).join(", ")} or ${last}`;
}

/** `Only admins can invite people to "work". Ask anna or bob to send an invite.` */
function inviteNoteText(net: string, admins: string[]): string {
  const ask = admins.length > 0 ? ` Ask ${orList(admins)} to send an invite.` : "";
  return `Only admins can invite people to "${net}".${ask}`;
}

/** The plaque for a slot, or `null` when this computer may invite (Peer network, admin, no snapshot yet). */
function inviteNoteOf(store: Store, slot: string): string | null {
  const net = stateOf(store, slot)?.network;
  const brief = store.nets.find((n) => n.id === slot);
  if (net?.kind !== "Managed" || (net.can_invite ?? brief?.can_invite) !== false) return null;
  return inviteNoteText(netName(store.nets, slot), adminNamesOf(store, slot) ?? []);
}

export const useInviteNote = (store: Store, slot: () => string): ComputedRef<string | null> => computed(() => inviteNoteOf(store, slot()));
