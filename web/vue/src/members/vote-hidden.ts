/* Which removal proposals the banner still shows after "Not now". Pure functions. */

import type { MemberRow } from "./members-types.ts";

// A proposal is told apart by the member to remove and the proposer.
export const proposalKey = (m: MemberRow): string => m.id_full + ":" + (m.vote ? m.vote.proposed_by : "");

export function pendingProposal(rows: MemberRow[], hidden: string[]): MemberRow | null {
  return rows.find((m) => !hidden.includes(proposalKey(m))) || null;
}

// Keys of proposals that are gone drop out, so a proposal that comes back is shown again.
export function pruneHidden(hidden: string[], rows: MemberRow[]): string[] {
  const alive = rows.map(proposalKey);
  return hidden.filter((key) => alive.includes(key));
}
