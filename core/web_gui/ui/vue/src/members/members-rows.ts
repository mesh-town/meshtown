/* What a member row shows (`rowDecor`) and what its `⋯` menu offers (`menuItems`). Pure functions. */

import { BADGE_BLOCKED, LINE_CANT_CONNECT } from "./members-texts.ts";
import { forDevice } from "../common/device.ts";
import { managedMenuItems, type MenuCtx } from "./members-managed-rows.ts";
import type { MemberRow, MenuItem } from "./members-types.ts";

export type { MenuCtx };

interface Badge {
  text: string;
  tone: "warn" | "grey";
}

interface RowDecor {
  badge: Badge | null;
  lines: string[];
  mainAction: "unblock" | null;
}

interface PeerLike {
  id_full: string;
  self?: boolean;
}

const NEUTRAL = (): RowDecor => ({ badge: null, lines: [], mainAction: null });
const blocked = (): Badge => ({ text: forDevice(BADGE_BLOCKED), tone: "grey" });

export function rowDecor(peer: PeerLike, m: MemberRow | null | undefined, me: string): RowDecor {
  if (!m || peer.self || m.id_full === me || m.self || m.status !== "member") return NEUTRAL();
  if (m.blocked_by_me) return { badge: blocked(), lines: [LINE_CANT_CONNECT], mainAction: "unblock" };
  return NEUTRAL();
}

export function menuItems(row: MemberRow | null | undefined, ctx: MenuCtx): MenuItem[] {
  if (!row || row.status === "excluded") return [];
  return managedMenuItems(row, ctx);
}
