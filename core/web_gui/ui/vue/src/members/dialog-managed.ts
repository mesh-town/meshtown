/* Dialogs of a managed network: roles, removal by an admin, deleting the network. */

import {
  DELETE_NET_FIELD, GIVE_UP_BODY, LAST_ADMIN_BODY, deleteNetBody, deleteNetExitLine, deleteNetTitle, giveUpTitle, lastAdminTitle, makeAdminBody,
  makeAdminTitle, removeBody, removeTitle,
} from "./members-managed-texts.ts";
import type { DialogContext, DialogSpec } from "./dialog-specs.ts";

export type ManagedKind = "make-admin" | "give-up" | "ban-managed" | "delete-net" | "last-admin";
type ManagedPart = Omit<DialogSpec, "danger">;

// The network name must be typed whole; a long name still fits the input.
const NET_NAME_MAX = 200;

const net = (c: DialogContext): string => c.net || "";

function deleteNet(c: DialogContext): ManagedPart {
  const exit = c.isMyExit && c.exitName ? [deleteNetExitLine(c.exitName)] : [];
  return {
    title: deleteNetTitle(net(c)),
    body: [deleteNetBody(c.members ?? 0, net(c)), ...exit],
    field: { label: DELETE_NET_FIELD, max: NET_NAME_MAX, match: net(c) },
    action: "Delete network",
  };
}

export const MANAGED_BUILDERS: Record<ManagedKind, (c: DialogContext) => ManagedPart> = {
  "make-admin": (c) => ({ title: makeAdminTitle(c.name, net(c)), body: [makeAdminBody(c.name)], action: "Make admin" }),
  "give-up": (c) => ({ title: giveUpTitle(net(c)), body: [GIVE_UP_BODY], action: "Give up" }),
  "ban-managed": (c) => ({ title: removeTitle(c.name, net(c)), body: [removeBody(c.name)], action: "Remove" }),
  "delete-net": deleteNet,
  "last-admin": (c) => ({
    title: lastAdminTitle(net(c)),
    body: [LAST_ADMIN_BODY],
    action: "Choose an admin",
    alt: { label: "Delete network…", danger: true },
  }),
};

const isManagedKind = (kind: string): kind is ManagedKind => kind in MANAGED_BUILDERS;

// Only a removal names a member who may be my exit; the other kinds do not cut anything off.
export const MANAGED_CALM: readonly string[] = ["make-admin", "give-up", "delete-net", "last-admin"];
export const MANAGED_CALM_DANGER: readonly string[] = ["make-admin", "last-admin"];
