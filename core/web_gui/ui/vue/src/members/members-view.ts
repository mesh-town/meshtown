/* Pure logic of the members part of the page; the row decoration and the `⋯` menu are in `members-rows.ts`,
 * lookups and the confirm rules in `members-ids.ts`. */

import { FORGET_WAITING, forgottenLine } from "./members-texts.ts";
import type { MemberRow, MembersReply } from "./members-types.ts";

export * from "./members-ids.ts";
export * from "./members-rows.ts";
export * from "./managed-ctx.ts";

export interface PeerLike {
  id: string;
  id_full: string;
  name: string | null;
  online: boolean;
  self?: boolean;
}

interface Partition<P> {
  active: (P | PeerLike)[];
  pending: MemberRow[];
  unlisted: MemberRow[];
  excluded: MemberRow[];
  forgotten: MemberRow[];
  stale: MemberRow[];
}

// A member offline for longer than this (or never seen) is listed in the "offline for over a week" group.
export const STALE_AFTER_SECS = 604800;

// Removed from the list, or hidden here because of my own pending removal vote.
export const inForgottenGroup = (m: MemberRow): boolean => m.status === "forgotten" || !!(m.forget && m.forget.mine);

export function isStale(m: MemberRow, me: string): boolean {
  if (m.status !== "member" || m.self || m.id_full === me || m.online || inForgottenGroup(m)) return false;
  return m.last_seen_sec === null || m.last_seen_sec > STALE_AFTER_SECS;
}

const minimalRow = (m: MemberRow): PeerLike => ({ id: m.id, id_full: m.id_full, name: m.name, online: m.online, self: m.self });
const EMPTY = { pending: [], unlisted: [], excluded: [], forgotten: [], stale: [] };

// Peers stay in `active` unless the list says the node is unlisted, excluded, stale or removed; list-only members are appended.
export function partitionRows<P extends PeerLike>(peers: P[] | null | undefined, members: MembersReply | null | undefined, me: string): Partition<P> {
  const list = peers || [];
  if (!members) return { active: list, ...EMPTY };
  const rows = members.members;
  const shown = new Set(rows.filter((m) => m.status === "member" && !inForgottenGroup(m) && !isStale(m, me)).map((m) => m.id_full));
  const known = new Set(list.map((p) => p.id_full));
  const listed = new Set(rows.map((m) => m.id_full));
  const kept = list.filter((p) => shown.has(p.id_full) || !listed.has(p.id_full));
  const extra = rows.filter((m) => shown.has(m.id_full) && !known.has(m.id_full)).map(minimalRow);
  const by = (status: string): MemberRow[] => rows.filter((m) => m.status === status);
  return {
    active: [...kept, ...extra],
    pending: by("pending"),
    unlisted: by("unlisted"),
    excluded: by("excluded"),
    forgotten: rows.filter(inForgottenGroup),
    stale: rows.filter((m) => isStale(m, me)),
  };
}

// "19 Oct": day without a leading zero and the English short month; not a number — "".
export function shortDate(unix: unknown, timeZone?: string): string {
  if (typeof unix !== "number" || !Number.isFinite(unix)) return "";
  const parts = new Intl.DateTimeFormat("en-US", { day: "numeric", month: "short", timeZone }).formatToParts(new Date(unix * 1000));
  const pick = (type: string): string => (parts.find((p) => p.type === type) || { value: "" }).value;
  return pick("day") + " " + pick("month");
}

const joinNames = (list: string[]): string => list.join(" and ");

// "{x} · banned on 19 Oct · by {a} and {b}" / "{x} · left the network on 19 Oct".
export function excludedRowLine(m: MemberRow, names: Record<string, string>, timeZone?: string): string {
  const who = m.name && m.name.trim() ? m.name : m.id;
  const ex = m.excluded;
  const date = ex ? shortDate(ex.at_unix, timeZone) : "";
  if (ex && ex.how === "left") return who + " · left the network on " + date;
  const by = ex ? joinNames(ex.voters.map((v) => names[v] || v.slice(0, 8))) : "";
  return who + " · banned on " + date + " · by " + by;
}

// A row of the "removed from the member list" group: a pending circle waits for one more vote;
// a complete one names the members who removed it, e.g. "Removed by A and B on 19 Oct".
export function forgottenRowLine(m: MemberRow, rows: MemberRow[], timeZone?: string): string {
  if (!m.forget || m.forget.at_unix === null) return FORGET_WAITING;
  const names = m.forget.voters.map((v) => {
    const row = rows.find((r) => r.id === v);
    return (row && row.name && row.name.trim()) || v;
  });
  return forgottenLine(joinNames(names), shortDate(m.forget.at_unix, timeZone));
}
