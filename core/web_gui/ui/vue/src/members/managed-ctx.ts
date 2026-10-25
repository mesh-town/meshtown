/* The managed-network facts a member menu and the leave dialog need, read from the snapshot and the member list. */

import type { NetworkInfo } from "../core/api-types.ts";
import { isRoot } from "./members-ids.ts";
import type { MenuCtx } from "./members-managed-rows.ts";
import type { MembersReply } from "./members-types.ts";

interface ManagedFacts extends MenuCtx {
  canLeave?: boolean;
}

const adminIds = (net: NetworkInfo | null | undefined, list: MembersReply | null | undefined): string[] => list?.admins ?? net?.admins ?? [];

// A peer network (no `kind` anywhere) gets `{ me, managed: false }`.
export function managedFacts(net: NetworkInfo | null | undefined, list: MembersReply | null | undefined, me: string): ManagedFacts {
  const managed = net?.kind === "Managed" || list?.kind === "Managed";
  if (!managed) return { me, managed: false };
  const meAdmin = list?.me_admin ?? net?.me_role === "admin";
  return { me, managed, meAdmin, onlyAdmin: meAdmin && adminIds(net, list).length <= 1, canLeave: net?.can_leave, isRoot: (id) => isRoot(net, id) };
}

// An admin may delete a Managed network for everyone; the node's `can_delete_network` decides when it says.
export function canDeleteNet(net: NetworkInfo | null | undefined): boolean {
  return net?.kind === "Managed" && (net.can_delete_network ?? net.me_role === "admin");
}
