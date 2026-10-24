/* Client of `GET /api/members` and `POST /api/members/<action>`. */

import type { ApiFn } from "../core/api.ts";
import { slotPath } from "../core/slots.ts";
import { parseOverview } from "../core/overview.ts";
import type { LeaveReply } from "../core/api-types.ts";
import { NET_NAME, errReason, failText } from "../common/words.ts";
import { cleanReason } from "./member-reason.ts";
import type { AdmissionInfo, ExcludedInfo, ForgetInfo, MemberAction, MemberCounts, MemberRow, MembersReply, MemberStatus, RefusalInfo, VoteInfo } from "./members-types.ts";

type Raw = Record<string, unknown>;

const STATUSES: readonly string[] = ["member", "pending", "unlisted", "excluded", "forgotten"];
const ACTION = "update the member list";

const isObj = (v: unknown): v is Raw => typeof v === "object" && v !== null && !Array.isArray(v);
const str = (v: unknown, dflt: string): string => (typeof v === "string" ? v : dflt);
const bool = (v: unknown): boolean => v === true;
const num = (v: unknown, dflt: number): number => (typeof v === "number" && Number.isFinite(v) ? v : dflt);
const ids = (v: unknown): string[] => (Array.isArray(v) ? v.filter((x): x is string => typeof x === "string") : []);

function parseVote(v: unknown): VoteInfo | null {
  if (!isObj(v)) return null;
  const reason = typeof v.reason === "string" && v.reason.trim() ? v.reason : null;
  return { votes: num(v.votes, 0), need: num(v.need, 2), proposed_by: str(v.proposed_by, ""), voters: ids(v.voters), mine: bool(v.mine), reason };
}

function parseAdmission(v: unknown): AdmissionInfo | null {
  if (!isObj(v)) return null;
  const label = typeof v.label === "string" && v.label.trim() ? v.label : null;
  return { votes: num(v.votes, 0), need: num(v.need, 2), voters: ids(v.voters), mine: bool(v.mine), label };
}

function parseExcluded(v: unknown): ExcludedInfo | null {
  if (!isObj(v)) return null;
  return { how: v.how === "left" ? "left" : "votes", at_unix: num(v.at_unix, 0), voters: ids(v.voters) };
}

function parseForget(v: unknown): ForgetInfo | null {
  if (!isObj(v)) return null;
  const at = typeof v.at_unix === "number" && Number.isFinite(v.at_unix) ? v.at_unix : null;
  return { votes: num(v.votes, 0), need: num(v.need, 2), proposed_by: str(v.proposed_by, ""), voters: ids(v.voters), mine: bool(v.mine), at_unix: at };
}

function parseRefusal(v: unknown): RefusalInfo | null {
  if (!isObj(v)) return null;
  return { side: v.side === "them" ? "them" : "me", reason: str(v.reason, ""), at_unix: num(v.at_unix, 0) };
}

function parseRow(v: unknown): MemberRow | null {
  if (!isObj(v) || typeof v.id_full !== "string" || !STATUSES.includes(v.status as string)) return null;
  const idFull = v.id_full;
  return {
    id: str(v.id, idFull.slice(0, 8).toLowerCase()),
    id_full: idFull,
    name: typeof v.name === "string" ? v.name : null,
    self: bool(v.self),
    online: bool(v.online),
    status: v.status as MemberStatus,
    vote: parseVote(v.vote),
    excluded: parseExcluded(v.excluded),
    blocked_by_me: bool(v.blocked_by_me),
    denied_by_me: typeof v.denied_by_me === "string" ? v.denied_by_me : null,
    last_refusal: parseRefusal(v.last_refusal),
    admission: parseAdmission(v.admission),
    forget: parseForget(v.forget),
    last_seen_sec: typeof v.last_seen_sec === "number" && Number.isFinite(v.last_seen_sec) ? v.last_seen_sec : null,
    ...(typeof v.admin === "boolean" ? { admin: v.admin } : {}),
  };
}

function parseCounts(raw: unknown, rows: MemberRow[]): MemberCounts {
  const c = isObj(raw) ? raw : {};
  const count = (pred: (r: MemberRow) => boolean): number => rows.filter(pred).length;
  return {
    members: num(c.members, count((r) => r.status === "member")),
    voting: num(c.voting, count((r) => r.vote !== null)),
    excluded: num(c.excluded, count((r) => r.status === "excluded")),
    unlisted: num(c.unlisted, count((r) => r.status === "unlisted")),
    pending: num(c.pending, count((r) => r.status === "pending")),
    forgotten: num(c.forgotten, count((r) => r.status === "forgotten")),
  };
}

// Tolerant to extra and missing fields; not a members list — null.
export function parseMembers(raw: unknown): MembersReply | null {
  if (!isObj(raw) || !Array.isArray(raw.members)) return null;
  const rows = raw.members.map(parseRow).filter((r): r is MemberRow => r !== null);
  const need = typeof raw.need === "number" && Number.isInteger(raw.need) && raw.need >= 1 ? raw.need : 2;
  const reply: MembersReply = {
    me: str(raw.me, ""),
    me_excluded: bool(raw.me_excluded),
    me_removed: bool(raw.me_removed),
    me_pending: bool(raw.me_pending),
    me_admission: parseAdmission(raw.me_admission),
    need,
    counts: parseCounts(raw.counts, rows),
    members: rows,
  };
  if (typeof raw.changed === "boolean") reply.changed = raw.changed;
  if (raw.kind === "Peer" || raw.kind === "Managed") reply.kind = raw.kind;
  if (Array.isArray(raw.admins)) reply.admins = ids(raw.admins);
  if (typeof raw.me_admin === "boolean") reply.me_admin = raw.me_admin;
  return reply;
}

export async function getMembers(api: ApiFn): Promise<MembersReply | null> {
  return parseMembers(await api("GET", "/api/members"));
}

function actionBody(action: MemberAction, id?: string, reason?: string): Record<string, unknown> {
  if (action === "leave") return { confirm: true };
  const clean = action === "exclude" && reason ? cleanReason(reason) : "";
  return clean ? { id, reason: clean } : { id };
}

export async function postMemberAction(api: ApiFn, action: MemberAction, id?: string, reason?: string, slot = "main"): Promise<{ reply: MembersReply; changed: boolean }> {
  const body = actionBody(action, id, reason);
  const parsed = parseMembers(await api("POST", slotPath(slot, "members/" + action), body));
  if (!parsed) throw new Error(NET_NAME + " sent an unreadable member list");
  const { changed, ...reply } = parsed;
  return { reply, changed: changed === true };
}

// `POST members/admin`: make a member an admin of a managed network, or a regular member again.
export async function postAdmin(api: ApiFn, idFull: string, admin: boolean, slot = "main"): Promise<{ reply: MembersReply; changed: boolean }> {
  const parsed = parseMembers(await api("POST", slotPath(slot, "members/admin"), { id: idFull, admin }));
  if (!parsed) throw new Error(NET_NAME + " sent an unreadable member list");
  const { changed, ...reply } = parsed;
  return { reply, changed: changed === true };
}

// `POST network/delete`: delete a managed network for everyone.
export async function deleteNetwork(api: ApiFn, slot: string): Promise<void> {
  await api("POST", slotPath(slot, "network/delete"), { confirm: true });
}

// The reply of `members/leave`: the member list plus `left`, `announced` and the briefs of the remaining networks.
// An old node sends only the list: a 200 then means "left".
export function parseLeave(raw: unknown): LeaveReply | null {
  const members = parseMembers(raw);
  if (!members) return null;
  const r = raw as Raw;
  const nets = parseOverview({ slots: [], nets: r.nets })?.nets ?? [];
  return { ...members, left: r.left !== false, announced: r.announced !== false, nets };
}

export async function leaveNet(api: ApiFn, slot: string): Promise<LeaveReply> {
  const parsed = parseLeave(await api("POST", slotPath(slot, "members/leave"), { confirm: true }));
  if (!parsed) throw new Error(NET_NAME + " sent an unreadable member list");
  return parsed;
}

// Codes of a managed network: whole sentences, no prefix.
const SENTENCES: Record<string, string> = {
  not_admin: "Only admins can do this.",
  last_admin: "You're the only admin: make another member an admin or delete the network.",
  not_managed: "This is a peer network: it has no admins.",
  peer_only: "This only works in a peer network.",
};

const REASONS: Record<string, string> = {
  self_target: "you can't remove your own computer",
  not_member: "that computer isn't in the member list",
  left: "this computer has left the network",
  busy: "the node is starting",
  invalid: "the member id is not valid",
  not_pending: "this node isn't waiting for confirmation",
  not_confirmed: "you haven't confirmed this node, or it has already joined",
  internal: "the node couldn't write the member list",
};

// "Couldn't update the member list: {reason}"; a bare machine code is replaced by a human reason.
export function memberErrorText(err: unknown): string {
  const code = isObj(err) && typeof err.code === "string" ? err.code : "";
  const reason = errReason(err);
  if (reason && reason !== code) return failText(ACTION, err);
  if (SENTENCES[code]) return SENTENCES[code];
  const known = REASONS[code];
  return known ? "Couldn't " + ACTION + ": " + known : failText(ACTION, null);
}
