/* Lookups and the "can I admit" rules over `GET /api/members`. Pure functions. */

import type { MemberRow, MembersReply } from "./members-types.ts";

interface StateWithNode {
  node?: { id_full?: string } | null;
  members?: { me_excluded?: boolean } | null;
}

export function memberById(members: MembersReply | null | undefined, idFull: string): MemberRow | null {
  if (!members) return null;
  return members.members.find((m) => m.id_full === idFull) || null;
}

export function meId(state: StateWithNode | null | undefined): string {
  return (state && state.node && state.node.id_full) || "";
}

// A join request from someone else that an admin can let in.
export function canAdmit(m: MemberRow | null | undefined, me: string): boolean {
  return !!m && m.status === "pending" && !m.self && m.id_full !== me;
}

export function admittable(members: MembersReply | null | undefined, me: string): MemberRow[] {
  if (!members) return [];
  return members.members.filter((m) => canAdmit(m, me));
}

export function isMeExcluded(state: StateWithNode | null | undefined, members: MembersReply | null | undefined): boolean {
  return !!(state && state.members && state.members.me_excluded) || !!(members && members.me_excluded);
}

// The only member besides me and `subject` (its full id), or null when there are more or fewer.
export function onlyOtherMember(members: MembersReply | null | undefined, me: string, subject: string): string | null {
  const rest = (members ? members.members : []).filter((m) => m.status === "member" && m.id_full !== me && m.id_full !== subject);
  return rest.length === 1 ? rest[0].id_full : null;
}

export type ExcludedVariant = "banned" | "left";

// "left" only when my own row says I left on my own.
export function excludedVariant(members: MembersReply | null | undefined, me: string): ExcludedVariant {
  const mine = memberById(members, me);
  return mine && mine.excluded && mine.excluded.how === "left" ? "left" : "banned";
}

// id_full -> name, or the short id for a nameless (or blank-named) member.
export function displayNames(members: MembersReply | null | undefined): Record<string, string> {
  const out: Record<string, string> = {};
  for (const m of members ? members.members : []) out[m.id_full] = m.name && m.name.trim() ? m.name : m.id;
  return out;
}

// This member created the network (`NetworkInfo.root`).
export function isRoot(info: { root?: string } | null | undefined, idFull: string): boolean {
  return !!(info && info.root && info.root === idFull);
}

// An admin who cannot leave (the only admin) must choose a successor first.
export function leaveKind(ctx: { meAdmin?: boolean; canLeave?: boolean; onlyAdmin?: boolean }): "leave" | "last-admin" {
  return ctx.meAdmin && (ctx.canLeave === false || ctx.onlyAdmin) ? "last-admin" : "leave";
}
