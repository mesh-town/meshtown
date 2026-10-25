/* The "the members turned this computer down" names over `GET /api/members`. Pure functions without Vue. */

import type { MembersReply } from "../members/members-types.ts";
import { displayNames } from "../members/members-view.ts";

/** The refusal to show: the names of the members that turned this computer down (maybe none are known). */
export type Refused = { names: string[] };

// Names of the other members that refused this computer when it came in. "Not listed yet" is normal for a while after
// joining: the list is still reaching the members, so it does not count.
export function refusingNames(members: MembersReply | null | undefined): string[] {
  if (!members) return [];
  const refusing = members.members.filter((m) => !m.self && m.last_refusal?.side === "them" && m.last_refusal.reason !== "unlisted");
  const names = displayNames(members);
  return refusing.map((m) => names[m.id_full]);
}

// The server decides whether the refusal stands (`flag`); the member list only supplies the names.
export function refusedNow(flag: boolean, members: MembersReply | null | undefined): Refused | null {
  return flag ? { names: refusingNames(members) } : null;
}
