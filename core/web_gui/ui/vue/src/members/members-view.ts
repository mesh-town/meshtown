/* Pure logic of the members part of the page; the row decoration and the `⋯` menu are in `members-rows.ts`,
 * lookups and the confirm rules in `members-ids.ts`. */

import type { MemberRow, MembersReply } from "./members-types.ts";

export * from "./members-ids.ts";
export * from "./members-rows.ts";

export interface PeerLike {
  id: string;
  id_full: string;
  name: string | null;
  online: boolean;
  self?: boolean;
}

export interface Partition<P> {
  active: (P | PeerLike)[];
  pending: MemberRow[];
  unlisted: MemberRow[];
  excluded: MemberRow[];
}

const minimalRow = (m: MemberRow): PeerLike => ({ id: m.id, id_full: m.id_full, name: m.name, online: m.online, self: m.self });

// Peers stay in `active` unless the list says the node is unlisted or excluded; list-only members are appended.
export function partitionRows<P extends PeerLike>(peers: P[] | null | undefined, members: MembersReply | null | undefined, me: string): Partition<P> {
  const list = peers || [];
  if (!members) return { active: list, pending: [], unlisted: [], excluded: [] };
  const status = new Map(members.members.map((m) => [m.id_full, m.status]));
  const known = new Set(list.map((p) => p.id_full));
  const kept = list.filter((p) => (status.get(p.id_full) || "member") === "member");
  const extra = members.members.filter((m) => m.status === "member" && !known.has(m.id_full)).map(minimalRow);
  return {
    active: [...kept, ...extra],
    pending: members.members.filter((m) => m.status === "pending"),
    unlisted: members.members.filter((m) => m.status === "unlisted"),
    excluded: members.members.filter((m) => m.status === "excluded"),
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
