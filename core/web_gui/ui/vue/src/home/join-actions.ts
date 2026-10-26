/* The join card of the Home tab without Vue: joining by invite, creating your own network. Errors are not thrown — the text goes to `form`.
 * The network key and the invite string go only into the request body and do not stay in `form`. */

import { watch } from "vue";
import { forDevice } from "../common/device.ts";
import type { CreateNetworkRequest, CreateNetworkResponse, JoinRequest, JoinResponse } from "../core/api-types.ts";
import type { ApiErrorLike } from "../core/api.ts";
import type { Store } from "../core/store.ts";
import { defaultHomeDeps, writeSetupMark } from "./home-actions.ts";
import type { HomeDeps, SetupMark } from "./home-actions.ts";
import { ADD_NET_LIMIT, createdBanner } from "./add-net-texts.ts";
import { failText, quotedNet, upperFirst } from "../common/words.ts";
import {
  HOME_CREATE_FAILED, HOME_INVITE_EMPTY, HOME_INVITE_INVALID, HOME_JOIN_ACTION, HOME_LABEL_EMPTY, JOIN_STALLED, JOIN_STALLED_SILENT,
  JOIN_WAITING, joinedStatusText, waitingStatusText,
} from "./home-flow-texts.ts";

const INVITE_EMPTY = HOME_INVITE_EMPTY;
const INVITE_INVALID = HOME_INVITE_INVALID;
const LABEL_EMPTY = HOME_LABEL_EMPTY;

// The deadline of the "the status will appear here" promise — headroom for the node restart. By its end the page must
// have a snapshot of a running node with a new `boot_id` (the node starts anew on joining a network), otherwise it is a failure.
// The page polls every 2 s on a visible tab and every 10 s on a hidden one, a request waits up to 5 s for a response; the
// "Mesh Town isn't responding" bar is the second failed poll in a row (`conn.failures >= 2`), unrelated to the deadline.
// Two texts: the node is silent (`store.absence`) — the node pulls the "Log" feed itself and there is nothing new in it, so
// we point to the node's log file; the node responds — we point to the tab. The tab is named in words: `form.status` is rendered as text, not markup.
export const JOIN_PROMISE_MS = 20000;
export { JOIN_STALLED, JOIN_STALLED_SILENT };

// Invite `mesh-invite2-` accepted, joining waits for members in the node (`node.invite_join`): no restart yet,
// the `JOIN_PROMISE_MS` deadline is not set.
export { JOIN_WAITING };

interface InviteError {
  message: string;
  word: string | null;
}
export interface JoinForm {
  inviteInput: string;
  inviteError: InviteError | null;
  label: string;
  labelError: string | null;
  choice: "" | "join" | "create";
  busy: "" | "join" | "create";
  status: string;
  /** The slot the last join went to, the slot of an "already in" answer, and the name the node gave the network. */
  slot?: string;
  dupSlot?: string;
  netLabel?: string;
}
export interface HighlightPart {
  text: string;
  highlighted: boolean;
}
export type JoinResult = "skipped" | "empty" | "joined" | "waiting" | "here" | "limit" | "created" | "failed";
type RequestBody = JoinRequest | CreateNetworkRequest;

export function createJoinForm(): JoinForm {
  return { inviteInput: "", inviteError: null, label: "", labelError: null, choice: "", busy: "", status: "" };
}

// Body of the word highlight in the key error without raw HTML: `[{ text, highlighted }]`.
export function highlightParts(message: string, word: string | null | undefined): HighlightPart[] {
  if (!word) return [{ text: message, highlighted: false }];
  const marker = '"' + word + '"';
  return markerParts(message, word, marker, message.indexOf(marker));
}

function markerParts(message: string, word: string, marker: string, idx: number): HighlightPart[] {
  if (idx < 0) return [{ text: message + " ", highlighted: false }, { text: word, highlighted: true }];
  return [
    { text: message.slice(0, idx), highlighted: false },
    { text: marker, highlighted: true },
    { text: message.slice(idx + marker.length), highlighted: false },
  ];
}

const asking = new WeakSet<JoinForm>();

export async function ask(form: JoinForm, run: () => Promise<JoinResult> | JoinResult): Promise<JoinResult> {
  if (form.busy || asking.has(form)) return "skipped";
  asking.add(form);
  try {
    return await run();
  } finally {
    asking.delete(form);
  }
}

async function request<T>(form: JoinForm, kind: "join" | "create", path: string, body: RequestBody, deps: HomeDeps): Promise<T> {
  form.busy = kind;
  try {
    return await deps.api("POST", path, body);
  } finally {
    form.busy = "";
  }
}

type ErrBody = { slot?: string; label?: string; message?: string; word?: string };

function joinFailed(form: JoinForm, err: ApiErrorLike): JoinResult {
  const body = (err.body || {}) as ErrBody;
  if (err.code === "net_already_added") return alreadyHere(form, body);
  if (err.code === "too_many_nets") return atTheLimit(form);
  const invalid = err.code === "invalid_key";
  const message = invalid ? body.message || INVITE_INVALID : failText(HOME_JOIN_ACTION, err);
  form.inviteError = { message, word: invalid ? body.word || null : null };
  return "failed";
}

function alreadyHere(form: JoinForm, body: ErrBody): JoinResult {
  form.dupSlot = body.slot;
  form.netLabel = body.label || undefined;
  return "here";
}

function atTheLimit(form: JoinForm): JoinResult {
  form.inviteError = { message: ADD_NET_LIMIT, word: null };
  return "limit";
}

// The "Three steps" setup — only for a node that joined its first network; switching from "Settings" (`mark` null) does not wake it.
export const markFor = (store: Store): SetupMark | null => (store.state && store.state.network ? null : "browser");

const writeMark = (mark: SetupMark | null, deps: HomeDeps) => mark && writeSetupMark(mark, deps.storage);

const slotOfPath = (path: string): string => /^\/api\/net\/([^/]+)\//.exec(path)?.[1] ?? "main";

// The slot of the answer: an older node does not name it, then it is the slot of the address.
function joinDone(form: JoinForm, res: JoinResponse | null, path: string): JoinResult {
  form.inviteInput = "";
  form.slot = res?.slot ?? slotOfPath(path);
  form.netLabel = res?.label || undefined;
  if (res && res.waiting === true) {
    form.status = res.label ? waitingStatusText(quotedNet(res.label)) : forDevice(JOIN_WAITING);
    return "waiting";
  }
  form.status = joinedStatusText(quotedNet(res && res.label));
  return "joined";
}

/** Clears what the last attempt left on the form and returns the trimmed key; an empty field puts the error on the form. */
export function beginJoin(form: JoinForm): string {
  const key = form.inviteInput.trim();
  form.status = "";
  form.slot = undefined;
  form.dupSlot = undefined;
  promiseTokenByForm.delete(form);
  stopLate(form);
  form.inviteError = key ? null : { message: INVITE_EMPTY, word: null };
  return key;
}

/** One request to a join address; the mark and the promise of a restart are not written here (see `afterMainJoin`). */
export async function postJoin(form: JoinForm, path: string, key: string, deps: HomeDeps): Promise<JoinResult> {
  try {
    return joinDone(form, await request<JoinResponse>(form, "join", path, { key }, deps), path);
  } catch (err) {
    return joinFailed(form, err as ApiErrorLike);
  }
}

/** A join into `main` restarts the node: the setup mark and the promise of the status. Another slot — the outcome as it is. */
export function afterMainJoin<T extends JoinResult>(form: JoinForm, store: Store, deps: HomeDeps, outcome: T, mark: SetupMark | null): T {
  if (form.slot !== "main") return outcome;
  if (outcome === "joined" || outcome === "waiting") writeMark(mark, deps);
  expectNode(form, store, deps, outcome);
  return outcome;
}

// A promise from `joinDone` waiting for its deadline. A new click clears the mark, and the old deadline stays silent.
const promiseTokenByForm = new WeakMap<JoinForm, symbol>();

// `boot_id` of the node's last snapshot; no snapshot or the field is not a string — null.
function storeBootId(store: Store): string | null {
  const boot = store.state?.boot_id;
  return typeof boot === "string" ? boot : null;
}

// The node works after joining: a new `boot_id`, not silent (`absence` is empty) and not `stopping` — a new start that has already
// stopped or gone silent does not count as an answer.
function answeredAfterJoin(store: Store, bootBeforeJoin: string | null): boolean {
  const now = storeBootId(store);
  return now !== null && now !== bootBeforeJoin && store.absence === null && store.state?.node?.status !== "stopping";
}

// A deadline failure waiting for a late snapshot. A new click cancels the watch.
const stopLateWatchByForm = new WeakMap<JoinForm, () => void>();

function stopLate(form: JoinForm): void {
  stopLateWatchByForm.get(form)?.();
  stopLateWatchByForm.delete(form);
}

// A deadline failure is not forever: a snapshot of a running node with a new `boot_id` arrives — the failure goes away, as with an answer before the deadline.
function liftOnAnswer(form: JoinForm, store: Store, bootBeforeJoin: string | null, refusal: string): void {
  const stop = watch(() => answeredAfterJoin(store, bootBeforeJoin), (answered) => {
    if (!answered) return;
    stopLate(form);
    if (form.status === refusal) form.status = "";
  }, { flush: "sync" });
  stopLateWatchByForm.set(form, stop);
}

function refuse(form: JoinForm, store: Store, bootBeforeJoin: string | null): void {
  form.status = store.absence ? JOIN_STALLED_SILENT : JOIN_STALLED;
  liftOnAnswer(form, store, bootBeforeJoin, form.status);
}

// The deadline expired: the node works after joining — the promise is removed, otherwise a failure replaces it (silent — with the log file),
// which a late snapshot removes. The promise is recognized by its mark and text.
function settlePromise(form: JoinForm, store: Store, token: symbol, text: string, bootBeforeJoin: string | null): void {
  if (promiseTokenByForm.get(form) !== token || form.status !== text) return;
  promiseTokenByForm.delete(form);
  if (answeredAfterJoin(store, bootBeforeJoin)) form.status = "";
  else refuse(form, store, bootBeforeJoin);
}

function expectNode(form: JoinForm, store: Store, deps: HomeDeps, outcome: JoinResult): JoinResult {
  if (outcome !== "joined") return outcome;
  const token = Symbol("join-promise");
  const text = form.status;
  const bootBeforeJoin = storeBootId(store);
  promiseTokenByForm.set(form, token);
  deps.setTimer(() => settlePromise(form, store, token, text, bootBeforeJoin), JOIN_PROMISE_MS);
  return outcome;
}

export function joinNetwork(form: JoinForm, store: Store, deps: HomeDeps = defaultHomeDeps): Promise<JoinResult> {
  return ask(form, () => startJoin(form, store, deps));
}

async function startJoin(form: JoinForm, store: Store, deps: HomeDeps): Promise<JoinResult> {
  const key = beginJoin(form);
  const mark = markFor(store);
  if (!key) return "empty";
  return afterMainJoin(form, store, deps, await postJoin(form, "/api/network/join", key, deps), mark);
}

function createFailed(form: JoinForm, err: ApiErrorLike): JoinResult {
  form.labelError = upperFirst(err.message) || HOME_CREATE_FAILED;
  return "failed";
}

// The key from the response is not read and is stored nowhere.
function createDone(form: JoinForm, res: CreateNetworkResponse | null, label: string, mark: SetupMark | null, deps: HomeDeps): JoinResult {
  form.label = "";
  form.choice = "";
  form.inviteInput = "";
  form.inviteError = null;
  const name = (res && res.label) || label;
  form.status = createdBanner(name);
  writeMark(mark, deps);
  return "created";
}

async function sendCreate(form: JoinForm, label: string, body: CreateNetworkRequest, mark: SetupMark | null, deps: HomeDeps): Promise<JoinResult> {
  try {
    return createDone(form, await request<CreateNetworkResponse>(form, "create", "/api/network/create", body, deps), label, mark, deps);
  } catch (err) {
    return createFailed(form, err as ApiErrorLike);
  }
}

export function createNetwork(form: JoinForm, store: Store, deps: HomeDeps = defaultHomeDeps): Promise<JoinResult> {
  return ask(form, () => startCreate(form, store, deps));
}

const createBody = (label: string): CreateNetworkRequest => ({ label });

function startCreate(form: JoinForm, store: Store, deps: HomeDeps): Promise<JoinResult> | "empty" {
  const label = form.label.trim();
  form.labelError = label ? null : LABEL_EMPTY;
  return label ? sendCreate(form, label, createBody(label), markFor(store), deps) : "empty";
}
