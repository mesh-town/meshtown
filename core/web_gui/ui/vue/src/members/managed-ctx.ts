/* The facts a member menu and the leave dialog need, read from the snapshot and the member list. */

import type { NetworkInfo } from "../core/api-types.ts";
import { isRoot } from "./members-ids.ts";
import type { MenuCtx } from "./members-managed-rows.ts";
import type { MembersReply } from "./members-types.ts";

interface ManagedFacts extends MenuCtx {
  adminIds: string[];
  canLeave?: boolean;
}

const adminIds = (net: NetworkInfo | null | undefined, list: MembersReply | null | undefined): string[] => list?.admins ?? net?.admins ?? [];

export function managedFacts(net: NetworkInfo | null | undefined, list: MembersReply | null | undefined, me: string): ManagedFacts {
  const meAdmin = list?.me_admin ?? net?.me_role === "admin";
  return { me, meAdmin, adminIds: adminIds(net, list), onlyAdmin: meAdmin && adminIds(net, list).length === 1, canLeave: net?.can_leave, isRoot: (id) => isRoot(net, id) };
}

// An admin may delete the network for everyone; the node's `can_delete_network` decides when it says.
export function canDeleteNet(net: NetworkInfo | null | undefined): boolean {
  return !!net && (net.can_delete_network ?? net.me_role === "admin");
}
