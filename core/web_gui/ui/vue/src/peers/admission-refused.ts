/* The "the members turned this computer down" rule over `GET /api/members`. A pure function without Vue. */

import type { MembersReply } from "../members/members-types.ts";
import { displayNames } from "../members/members-view.ts";

// Names of the members that refused this computer when it came in, `null` when the plate does not apply: no refusal from
// them, somebody is online, or the computer itself is excluded, removed or waiting for votes (those have their own cards).
export function admissionRefused(members: MembersReply | null | undefined): { names: string[] } | null {
  if (!members || members.me_excluded || members.me_removed || members.me_pending) return null;
  const others = members.members.filter((m) => !m.self);
  if (others.some((m) => m.online)) return null;
  // "Not listed yet" is normal for a while after joining: the list is still reaching the members.
  const refusing = others.filter((m) => m.last_refusal?.side === "them" && m.last_refusal.reason !== "unlisted");
  if (refusing.length === 0) return null;
  const names = displayNames(members);
  return { names: refusing.map((m) => names[m.id_full]) };
}
