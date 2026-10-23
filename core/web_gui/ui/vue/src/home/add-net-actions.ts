/* The "Add another network" form without Vue: claim an empty slot (`POST /api/nets`), then join or create in it.
 * Errors are not thrown: the text goes to `form`, the outcome is the return value. The invite goes only into the request body. */

import { notifyChanged } from "../core/bridge.ts";
import type { ApiErrorLike } from "../core/api.ts";
import type { JoinResponse, CreateNetworkResponse } from "../core/api-types.ts";
import { netName, slotPath, visibleSlots } from "../core/slots.ts";
import type { Store } from "../core/store.ts";
import { chosenExitIds } from "../common/exits.ts";
import { peerLabel, failText, upperFirst } from "../common/words.ts";
import { PEERS_NEW_NET } from "../peers/peers-texts.ts";
import type { HomeDeps } from "./home-actions.ts";
import { HOME_CREATE_FAILED, HOME_INVITE_EMPTY, HOME_INVITE_INVALID, HOME_LABEL_EMPTY } from "./home-flow-texts.ts";
import type { JoinForm } from "./join-actions.ts";
import { ADD_NET_ACTION, MAX_NETS, createdBanner, joinedBanner } from "./add-net-texts.ts";

export { stayText } from "./add-net-texts.ts";

export type AddNetResult = "joined" | "waiting" | "created" | "duplicate" | "limit" | "failed" | "empty" | "skipped";
export type AddNetMode = "join" | "create";

/** The reply of `POST /api/nets`: the slot and whether it was added just now (`fresh`) or handed back. */
interface TakenSlot {
  slot: string;
  fresh?: boolean;
}

/** `JoinForm` plus what the add form needs: the slot that holds the network, its name, the slot of a duplicate. */
export interface AddNetForm extends JoinForm {
  dupSlot?: string;
  slot?: string;
  netLabel?: string;
}

export const atLimit = (store: Store): boolean => visibleSlots(store.nets, store.slots).length >= MAX_NETS;

const errBody = (err: ApiErrorLike): { slot?: string; message?: string; word?: string } => (err.body || {}) as { slot?: string; message?: string; word?: string };

function clearForm(form: AddNetForm): void {
  form.status = "";
  form.dupSlot = undefined;
  form.slot = undefined;
  form.netLabel = undefined;
  form.inviteError = null;
  form.labelError = null;
}

// Empty input is the one local refusal: nothing is sent.
function refuseEmpty(form: AddNetForm, mode: AddNetMode): boolean {
  if (mode === "join" && !form.inviteInput.trim()) form.inviteError = { message: HOME_INVITE_EMPTY, word: null };
  if (mode === "create" && !form.label.trim()) form.labelError = HOME_LABEL_EMPTY;
  return mode === "join" ? !form.inviteInput.trim() : !form.label.trim();
}

function joinFailed(form: AddNetForm, err: ApiErrorLike): AddNetResult {
  const body = errBody(err);
  if (err.code === "net_already_added") {
    form.dupSlot = body.slot;
    return "duplicate";
  }
  if (err.code === "too_many_nets") return "limit";
  const invalid = err.code === "invalid_key";
  form.inviteError = { message: invalid ? body.message || HOME_INVITE_INVALID : failText(ADD_NET_ACTION, err), word: invalid ? body.word || null : null };
  return "failed";
}

function createFailed(form: AddNetForm, err: ApiErrorLike): AddNetResult {
  if (err.code === "too_many_nets") return "limit";
  form.labelError = upperFirst(err.message) || HOME_CREATE_FAILED;
  return "failed";
}

const failedAdd = (form: AddNetForm, mode: AddNetMode, err: unknown): AddNetResult =>
  mode === "join" ? joinFailed(form, err as ApiErrorLike) : createFailed(form, err as ApiErrorLike);

// A refused add leaves the slot it took empty; a slot that was new for this add is given back (the same route as "leave" for a
// slot without a network). A slot handed back from an earlier attempt, `main` and a pending or finished add stay.
function takenForNothing(taken: TakenSlot, result: AddNetResult): boolean {
  return taken.fresh === true && taken.slot !== "main" && (result === "failed" || result === "duplicate");
}

async function giveBack(form: AddNetForm, deps: HomeDeps, slot: string): Promise<void> {
  form.slot = undefined;
  try {
    await deps.api("POST", slotPath(slot, "members/leave"), { confirm: true });
  } catch {
    // The empty slot stays on the node; the next add takes it again.
  }
}

async function send(form: AddNetForm, deps: HomeDeps, mode: AddNetMode): Promise<AddNetResult> {
  const taken = await deps.api<TakenSlot>("POST", "/api/nets", {});
  form.slot = taken.slot;
  try {
    if (mode === "create") return created(form, await deps.api<CreateNetworkResponse>("POST", slotPath(taken.slot, "network/create"), { label: form.label.trim() }));
    return joined(form, await deps.api<JoinResponse>("POST", slotPath(taken.slot, "network/join"), { key: form.inviteInput.trim() }));
  } catch (err) {
    const result = failedAdd(form, mode, err);
    if (takenForNothing(taken, result)) await giveBack(form, deps, taken.slot);
    return result;
  }
}

function joined(form: AddNetForm, res: JoinResponse | null): AddNetResult {
  form.inviteInput = "";
  form.netLabel = (res && res.label) || undefined;
  return res && res.waiting === true ? "waiting" : "joined";
}

function created(form: AddNetForm, res: CreateNetworkResponse | null): AddNetResult {
  form.netLabel = (res && res.label) || form.label.trim();
  form.label = "";
  return "created";
}

export async function addNetwork(form: AddNetForm, store: Store, deps: HomeDeps, mode: AddNetMode): Promise<AddNetResult> {
  if (form.busy) return "skipped";
  clearForm(form);
  if (refuseEmpty(form, mode)) return "empty";
  if (atLimit(store)) return "limit";
  form.busy = mode;
  try {
    return await send(form, deps, mode);
  } catch (err) {
    return failedAdd(form, mode, err);
  } finally {
    form.busy = "";
    if (typeof document !== "undefined") notifyChanged(document, "state");
  }
}

// The exit the computer keeps using, when it is a member (not "directly"): shown in the banner so a new network does not look like a change.
export function currentExitVia(store: Store): { peer: string; net: string } | null {
  const id = chosenExitIds(store)[0];
  const peers = store.state?.peers ?? [];
  const peer = id ? peers.find((p) => p.id_full === id) : undefined;
  return peer ? { peer: peerLabel(peer, true, peers), net: netName(store.nets, store.lead) } : null;
}

// The name the tab shows: from the slot's entry when the poll already has its label, else the node's reply, else the slot's own fallback name.
function joinedName(store: Store, form: AddNetForm): string {
  const slot = form.slot ?? "";
  if (store.nets.some((n) => n.id === slot && n.label != null)) return netName(store.nets, slot);
  return form.netLabel || netName(store.nets, slot);
}

/** The banner text for a finished add; `null` — nothing to say (the join is still waiting, or it failed). */
export function addedBannerText(store: Store, form: AddNetForm, result: AddNetResult): string | null {
  if (result === "created") return createdBanner(form.netLabel || PEERS_NEW_NET);
  return result === "joined" ? joinedBanner(joinedName(store, form), currentExitVia(store)) : null;
}

/** The banner for a network that finished joining after a wait. */
export function arrivedBannerText(store: Store, slot: string): string {
  return joinedBanner(netName(store.nets, slot), currentExitVia(store));
}
