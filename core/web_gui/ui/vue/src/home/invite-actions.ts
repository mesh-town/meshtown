/* Invite of the Home tab without Vue. The invite string is a network secret: it lives only in `inv.text`
 * (a component `ref`) and does not reach the store, storages, `title` or the log. */

import type { InviteResponse } from "../core/api-types.ts";
import { slotPath, stateOf } from "../core/slots.ts";
import type { Store } from "../core/store.ts";
import { defaultHomeDeps } from "./home-actions.ts";
import type { HomeDeps } from "./home-actions.ts";
import { writeClipboard } from "../common/clipboard.ts";
import { forDevice } from "../common/device.ts";
import {
  HOME_INVITE_COPIED, HOME_INVITE_MANUAL, HOME_INVITE_STALE, HOME_INVITE_UNAVAILABLE, HOME_REACH_ISSUER_AND_ONE, HOME_REACH_ISSUER_ONLY,
} from "./home-flow-texts.ts";

const UNAVAILABLE = HOME_INVITE_UNAVAILABLE;
const COPIED = HOME_INVITE_COPIED;
const MANUAL = HOME_INVITE_MANUAL;
const STALE = HOME_INVITE_STALE;

export interface InviteState {
  text: string;
  netId: string | null;
  busy: boolean;
  status: string;
  warning: string;
}
export type InviteResult = "stale" | "copied" | "manual" | "failed" | "skipped";
type InviteDeps = Pick<HomeDeps, "api" | "clipboard">;

export function createInviteState(): InviteState {
  return { text: "", netId: null, busy: false, status: "", warning: "" };
}

function currentNetId(store: Store, slot: string): string | null {
  const net = stateOf(store, slot)?.network;
  return net ? net.id_hex : null;
}

// Reach warning; an unknown value and `spread` produce no text.
export function reachWarning(reach: unknown): string {
  if (reach === "issuer_only") return forDevice(HOME_REACH_ISSUER_ONLY);
  if (reach === "issuer_and_one") return forDevice(HOME_REACH_ISSUER_AND_ONE);
  return "";
}

// The response is applied only if the network in the current snapshot is the same as at request time.
async function showInvite(inv: InviteState, store: Store, slot: string, netId: string, res: InviteResponse, deps: InviteDeps): Promise<InviteResult> {
  if (currentNetId(store, slot) !== netId) {
    inv.status = STALE;
    return "stale";
  }
  inv.text = res.invite;
  inv.warning = reachWarning(res.reach);
  inv.netId = netId;
  const copied = await writeClipboard(deps, res.invite);
  inv.status = inv.netId === netId ? (copied ? COPIED : MANUAL) : inv.status;
  return copied ? "copied" : "manual";
}

// The invite path; a non-blank label travels as `?for=`.
export function invitePath(forWhom: string, slot = "main"): string {
  const label = forWhom.trim();
  return slotPath(slot, "network/invite") + (label ? "?for=" + encodeURIComponent(label) : "");
}

async function fetchInvite(inv: InviteState, store: Store, slot: string, netId: string, deps: InviteDeps, forWhom: string): Promise<InviteResult> {
  // A failed request gives the same text as an empty response.
  const res = await deps.api<InviteResponse>("GET", invitePath(forWhom, slot)).catch(() => null);
  if (res && res.invite) return showInvite(inv, store, slot, netId, res, deps);
  inv.status = UNAVAILABLE;
  return "failed";
}

// `netId` is taken BEFORE `await`; a response for another network is dropped ("stale").
export async function requestInvite(inv: InviteState, store: Store, deps: InviteDeps = defaultHomeDeps, forWhom = "", slot = store.lead): Promise<InviteResult> {
  const netId = currentNetId(store, slot);
  if (inv.busy || netId === null) return "skipped";
  inv.busy = true;
  try {
    return await fetchInvite(inv, store, slot, netId, deps, forWhom);
  } finally {
    inv.busy = false;
  }
}

// Another `id_hex` or a vanished network — clear the string and the status (`id_hex` is exposed in /api/state).
export function resetInviteIfNetChanged(inv: InviteState, netId: string | null | undefined): void {
  if (netId != null && netId === inv.netId) return;
  inv.text = "";
  inv.netId = null;
  inv.status = "";
  inv.warning = "";
}
