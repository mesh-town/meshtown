/* The one "Join with an invite" form without Vue: where a pasted string goes and what the page says about the place it leads to.
 * Errors are not thrown: the text goes to `form`, the outcome is the return value. The string goes only into the request body. */

import type { InspectResponse, NetBrief } from "../core/api-types.ts";
import { netName, slotPath } from "../core/slots.ts";
import type { HomeDeps } from "./home-actions.ts";
import { joinOnNewSlot } from "./add-net-actions.ts";
import type { AddNetResult } from "./add-net-actions.ts";
import { ask, beginJoin, postJoin } from "./join-actions.ts";
import type { JoinForm, JoinResult } from "./join-actions.ts";
import { ADD_NET_LIMIT_HERE, NOTE_BANNED_PEER, NOTE_EXCLUDED_MANAGED, NOTE_HERE, NOTE_NEW_AT_LIMIT, NOTE_REMOVED, NOTE_THIS_NET } from "./invite-join-texts.ts";

export type InviteOutcome = "joined" | "waiting" | "here" | "limit" | "failed" | "empty" | "skipped";
export type InviteFormState = JoinForm;
/** Where the form lives: Home (the `main` slot), "Add another network" (a slot of its own), the tab of a network the computer left. */
export interface InviteContext {
  kind: "home" | "add" | "rejoin";
  slot?: string;
}

/** An invite, as opposed to a word key. */
export const isInvite = (text: string): boolean => text.trim().startsWith("mesh-invite");

const OUTCOME: Record<JoinResult | AddNetResult, InviteOutcome> = {
  joined: "joined", waiting: "waiting", here: "here", duplicate: "here", limit: "limit", failed: "failed",
  empty: "empty", skipped: "skipped", created: "failed",
};

// A word key: "Add" claims a slot of its own, the tab of a left network joins in its own slot, Home joins in `main`.
function wordKey(form: InviteFormState, ctx: InviteContext, key: string, deps: HomeDeps): Promise<JoinResult | AddNetResult> {
  if (ctx.kind === "add") return joinOnNewSlot(form, deps);
  const tail = ctx.kind === "rejoin" && ctx.slot ? slotPath(ctx.slot, "network/join") : "/api/network/join";
  return postJoin(form, tail, key, deps);
}

async function route(form: InviteFormState, ctx: InviteContext, deps: HomeDeps): Promise<InviteOutcome> {
  const key = beginJoin(form);
  if (!key) return "empty";
  const sent = isInvite(key) ? postJoin(form, "/api/network/join", key, deps) : wordKey(form, ctx, key, deps);
  const outcome = OUTCOME[await sent];
  if (outcome === "limit" && ctx.kind === "add") form.inviteError = { message: ADD_NET_LIMIT_HERE, word: null };
  return outcome;
}

/** An invite goes to `/api/network/join` without a slot (the node picks it); a word key — by the context. */
export function sendInvite(form: InviteFormState, ctx: InviteContext, deps: HomeDeps): Promise<InviteOutcome> {
  return ask(form, () => route(form, ctx, deps)).then((res) => OUTCOME[res]);
}

/** The name of a network for a line of text: the tab's own name, else the node's, else a placeholder. */
export function netTitle(nets: NetBrief[], slot: string | undefined, label: string | null | undefined): string {
  return slot && nets.some((n) => n.id === slot) ? netName(nets, slot) : label || NOTE_THIS_NET;
}

function returnNote(reply: InspectResponse, nets: NetBrief[], name: string): string {
  const excluded = nets.some((n) => n.id === reply.slot && n.me_excluded);
  if (!excluded) return NOTE_REMOVED(name);
  return reply.kind === "Managed" ? NOTE_EXCLUDED_MANAGED(name) : NOTE_BANNED_PEER(name);
}

/** The line under the field about the place the invite leads to; empty for a new network (unless the node is at its limit) and for a key. */
export function placeNote(reply: InspectResponse, nets: NetBrief[], atLimit = false, kind: InviteContext["kind"] = "home"): string {
  const name = netTitle(nets, reply.slot, reply.label);
  if (reply.place === "here") return NOTE_HERE(name);
  if (reply.place === "return") return returnNote(reply, nets, name);
  return reply.place === "new" && atLimit && kind !== "add" ? NOTE_NEW_AT_LIMIT : "";
}
