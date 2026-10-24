/* Lookups and the "can I confirm" rules over `GET /api/members`. Pure functions. */

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

// Someone else's pending vote on a member who is neither me nor the proposer, and I have not voted yet.
export function canConfirm(m: MemberRow | null | undefined, me: string): boolean {
  if (!m || !m.vote) return false;
  return m.status === "member" && !m.vote.mine && m.vote.proposed_by !== me && m.id_full !== me;
}

export function confirmable(members: MembersReply | null | undefined, me: string): MemberRow[] {
  if (!members) return [];
  return members.members.filter((m) => canConfirm(m, me));
}

// A join request from someone else that I have not confirmed yet.
export function canAdmit(m: MemberRow | null | undefined, me: string): boolean {
  if (!m || m.status !== "pending" || !m.admission) return false;
  return !m.admission.mine && !m.self && m.id_full !== me;
}

// I confirmed this node and it has not reached the quorum: the confirmation can be withdrawn.
export function canUnadmit(m: MemberRow | null | undefined): boolean {
  return !!(m && m.admission && m.admission.mine && !m.self);
}

export function admittable(members: MembersReply | null | undefined, me: string): MemberRow[] {
  if (!members) return [];
  return members.members.filter((m) => canAdmit(m, me));
}

export function isMeExcluded(state: StateWithNode | null | undefined, members: MembersReply | null | undefined): boolean {
  return !!(state && state.members && state.members.me_excluded) || !!(members && members.me_excluded);
}

// A ban vote is pending on someone else and removing `subject` leaves two members or fewer: the threshold drops to one.
export function banAtRisk(members: MembersReply | null | undefined, subject: string): boolean {
  if (!members) return false;
  const voting = members.members.some((m) => m.id_full !== subject && m.vote !== null);
  return voting && members.members.filter((m) => m.status === "member" && m.id_full !== subject).length <= 2;
}

// The only member besides me and `subject` (its full id), or null when there are more or fewer.
export function onlyOtherMember(members: MembersReply | null | undefined, me: string, subject: string): string | null {
  const rest = (members ? members.members : []).filter((m) => m.status === "member" && m.id_full !== me && m.id_full !== subject);
  return rest.length === 1 ? rest[0].id_full : null;
}

// This computer was removed from the member list by the others (`me_removed`).
export function isMeRemoved(members: MembersReply | null | undefined): boolean {
  return !!(members && members.me_removed);
}

export type ExcludedVariant = "banned" | "left" | "forgotten";

// "left" only when my own row says I left on my own; a ban comes before a removal.
export function excludedVariant(members: MembersReply | null | undefined, me: string): ExcludedVariant {
  const mine = memberById(members, me);
  if (mine && mine.excluded && mine.excluded.how === "left") return "left";
  const banned = !!(mine && mine.excluded) || !!(members && members.me_excluded);
  return isMeRemoved(members) && !banned ? "forgotten" : "banned";
}

// id_full -> name, or the short id for a nameless (or blank-named) member.
export function displayNames(members: MembersReply | null | undefined): Record<string, string> {
  const out: Record<string, string> = {};
  for (const m of members ? members.members : []) out[m.id_full] = m.name && m.name.trim() ? m.name : m.id;
  return out;
}

// This member created the network (`NetworkInfo.root`); a peer network names no creator.
export function isRoot(info: { root?: string } | null | undefined, idFull: string): boolean {
  return !!(info && info.root && info.root === idFull);
}

// An admin who cannot leave (the only admin) must choose a successor first.
export function leaveKind(ctx: { meAdmin?: boolean; canLeave?: boolean; onlyAdmin?: boolean }): "leave" | "last-admin" {
  return ctx.meAdmin && (ctx.canLeave === false || ctx.onlyAdmin) ? "last-admin" : "leave";
}

// A managed network has no votes: the vote banner and the vote texts stay hidden.
export function votingHidden(reply: { kind?: string } | null | undefined): boolean {
  return !!reply && reply.kind === "Managed";
}
