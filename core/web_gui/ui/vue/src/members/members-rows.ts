/* What a member row shows (`rowDecor`) and what its `⋯` menu offers (`menuItems`). Pure functions. */

import {
  BADGE_BLOCKED, BADGE_PENDING, BADGE_BAN_PROPOSED, LINE_CANT_CONNECT, MENU_BLOCK, MENU_LEAVE, MENU_BAN, MENU_RENAME,
  MENU_UNADMIT, MENU_UNBLOCK, MENU_WAITING, MENU_WITHDRAW, TITLE_BLOCKED, TITLE_BAN_PROPOSED, admissionLine, proposedByText, waitingMoreText,
} from "./members-texts.ts";
import { canConfirm, canUnadmit } from "./members-ids.ts";
import type { MemberRow, MenuItem } from "./members-types.ts";

export interface Badge {
  text: string;
  tone: "warn" | "grey";
}

export interface RowDecor {
  badge: Badge | null;
  lines: string[];
  mainAction: "confirm" | "unblock" | null;
}

interface PeerLike {
  id_full: string;
  self?: boolean;
}

const NEUTRAL = (): RowDecor => ({ badge: null, lines: [], mainAction: null });
const PROPOSED: Badge = { text: BADGE_BAN_PROPOSED, tone: "warn" };
const BLOCKED: Badge = { text: BADGE_BLOCKED, tone: "grey" };

const nameOf = (names: Record<string, string>, id: string): string => names[id] || id.slice(0, 8);

function decorVote(m: MemberRow, me: string, names: Record<string, string>): RowDecor {
  const vote = m.vote!;
  const more = vote.need - vote.votes;
  if (vote.mine) return { badge: PROPOSED, lines: [waitingMoreText(more), BADGE_BLOCKED], mainAction: null };
  const mainAction = canConfirm(m, me) ? "confirm" : null;
  return { badge: PROPOSED, lines: [proposedByText(nameOf(names, vote.proposed_by), more)], mainAction };
}

export function rowDecor(peer: PeerLike, m: MemberRow | null | undefined, me: string, names: Record<string, string> = {}): RowDecor {
  if (!m || peer.self || m.id_full === me || m.self || m.status !== "member") return NEUTRAL();
  if (m.vote) return decorVote(m, me, names);
  if (m.admission) return { badge: { text: BADGE_PENDING, tone: "warn" }, lines: [admissionLine(m.admission.votes, m.admission.need)], mainAction: null };
  if (m.blocked_by_me) return { badge: BLOCKED, lines: [LINE_CANT_CONNECT], mainAction: "unblock" };
  return NEUTRAL();
}

const item = (key: MenuItem["key"], label: string, extra: Partial<MenuItem> = {}): MenuItem => ({ key, label, ...extra });
const disabled = (title: string): Partial<MenuItem> => ({ disabled: true, title });

function blockItem(m: MemberRow): MenuItem {
  if (m.blocked_by_me && m.status !== "member") return item("unblock", MENU_UNBLOCK);
  return item("block", MENU_BLOCK, m.blocked_by_me ? disabled(TITLE_BLOCKED) : {});
}

function banItem(m: MemberRow): MenuItem {
  return item("exclude", MENU_BAN, { danger: true, ...(m.vote ? disabled(TITLE_BAN_PROPOSED) : {}) });
}

function votedItems(row: MemberRow, me: string): MenuItem[] {
  if (row.vote!.proposed_by === me) return [item("withdraw", MENU_WITHDRAW)];
  return [item("waiting", MENU_WAITING, { disabled: true })];
}

// The extra item for a row I have confirmed: withdraw the confirmation.
const unadmitItems = (row: MemberRow): MenuItem[] => (canUnadmit(row) ? [item("unadmit", MENU_UNADMIT)] : []);

export function menuItems(row: MemberRow | null | undefined, ctx: { me: string }): MenuItem[] {
  if (!row || row.status === "excluded") return [];
  if (row.self || row.id_full === ctx.me) return [item("rename", MENU_RENAME), item("leave", MENU_LEAVE, { danger: true })];
  if (row.status === "unlisted") return [blockItem(row)];
  if (row.status === "pending") return [...unadmitItems(row), blockItem(row)];
  if (row.vote && row.vote.mine) return votedItems(row, ctx.me);
  return [blockItem(row), banItem(row), ...unadmitItems(row)];
}
