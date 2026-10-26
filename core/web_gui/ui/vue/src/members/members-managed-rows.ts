/* The `⋯` menu of a row of a managed network: roles and blocks. Pure functions. */

import {
  MENU_DELETE_NETWORK, MENU_GIVE_UP, MENU_GIVE_UP_ONLY, MENU_MAKE_ADMIN, MENU_MAKE_MEMBER, MENU_REMOVE, MENU_REMOVE_ADMINS_ONLY, TITLE_ADMINS_ONLY,
  TITLE_CREATOR, TITLE_ONLY_ADMIN,
} from "./members-managed-texts.ts";
import { forDevice } from "../common/device.ts";
import { MENU_BLOCK, MENU_LEAVE, MENU_RENAME, MENU_UNBLOCK, TITLE_BLOCKED } from "./members-texts.ts";
import type { MemberRow, MenuItem } from "./members-types.ts";

export interface MenuCtx {
  me: string;
  managed?: boolean;
  meAdmin?: boolean;
  onlyAdmin?: boolean;
  isRoot?: (idFull: string) => boolean;
}

const item = (key: MenuItem["key"], label: string, extra: Partial<MenuItem> = {}): MenuItem => ({ key, label, ...extra });

function blockItem(row: MemberRow): MenuItem {
  if (row.blocked_by_me && row.status !== "member") return item("unblock", MENU_UNBLOCK);
  const off = row.blocked_by_me ? { disabled: true, title: TITLE_BLOCKED } : {};
  return item("block", forDevice(MENU_BLOCK), off);
}

function roleItem(row: MemberRow, ctx: MenuCtx): MenuItem {
  if (!row.admin) return item("make-admin", MENU_MAKE_ADMIN);
  const creator = !!ctx.isRoot && ctx.isRoot(row.id_full);
  return item("make-member", MENU_MAKE_MEMBER, creator ? { disabled: true, title: TITLE_CREATOR } : {});
}

function othersItems(row: MemberRow, ctx: MenuCtx): MenuItem[] {
  if (row.status !== "member") return [blockItem(row)];
  if (ctx.meAdmin) return [roleItem(row, ctx), item("exclude", MENU_REMOVE, { danger: true }), blockItem(row)];
  return [blockItem(row), item("exclude", MENU_REMOVE_ADMINS_ONLY, { disabled: true, title: TITLE_ADMINS_ONLY })];
}

function giveUpItem(ctx: MenuCtx): MenuItem {
  if (ctx.onlyAdmin) return item("give-up", MENU_GIVE_UP_ONLY, { disabled: true, title: TITLE_ONLY_ADMIN });
  return item("give-up", MENU_GIVE_UP);
}

function ownItems(ctx: MenuCtx): MenuItem[] {
  const leave = item("leave", MENU_LEAVE, { danger: true });
  if (!ctx.meAdmin) return [item("rename", MENU_RENAME), leave];
  return [item("rename", MENU_RENAME), giveUpItem(ctx), leave, item("delete-network", MENU_DELETE_NETWORK, { danger: true })];
}

export function managedMenuItems(row: MemberRow, ctx: MenuCtx): MenuItem[] {
  if (row.self || row.id_full === ctx.me) return ownItems(ctx);
  return othersItems(row, ctx);
}
