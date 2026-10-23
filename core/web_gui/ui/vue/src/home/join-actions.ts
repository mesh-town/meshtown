/* The join card of the Home tab without Vue: joining by invite, creating your own network. Errors are not thrown — the text goes to `form`.
 * The network key and the invite string go only into the request body and do not stay in `form`. */

import { watch } from "vue";
import type { CreateNetworkRequest, CreateNetworkResponse, JoinRequest, JoinResponse } from "../core/api-types.ts";
import type { ApiErrorLike } from "../core/api.ts";
import type { Store } from "../core/store.ts";
import { defaultHomeDeps, writeSetupMark } from "./home-actions.ts";
import type { HomeDeps, SetupMark } from "./home-actions.ts";
import { failText, quotedNet, upperFirst } from "../common/words.ts";
import {
  HOME_CREATE_FAILED, HOME_INVITE_EMPTY, HOME_INVITE_INVALID, HOME_JOIN_ACTION, HOME_LABEL_EMPTY, JOIN_STALLED, JOIN_STALLED_SILENT,
  JOIN_WAITING, createdNoticeText, joinedStatusText,
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

export interface InviteError {
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
}
export interface HighlightPart {
  text: string;
  highlighted: boolean;
}
export type JoinResult = "skipped" | "empty" | "joined" | "waiting" | "created" | "failed";
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

// Text for `form.status` after creating a network.
export function createdNotice(label: unknown): string {
  return createdNoticeText(quotedNet(label));
}

const asking = new WeakSet<JoinForm>();

async function ask(form: JoinForm, run: () => Promise<JoinResult> | JoinResult): Promise<JoinResult> {
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

function joinFailed(form: JoinForm, err: ApiErrorLike): JoinResult {
  const body = err.body || {};
  const invalid = err.code === "invalid_key";
  const message = invalid ? body.message || INVITE_INVALID : failText(HOME_JOIN_ACTION, err);
  form.inviteError = { message, word: invalid ? body.word || null : null };
  return "failed";
}

// The "Three steps" setup — only for a node that joined its first network; switching from "Settings" (`mark` null) does not wake it.
const markFor = (store: Store): SetupMark | null => (store.state && store.state.network ? null : "browser");

const writeMark = (mark: SetupMark | null, deps: HomeDeps) => mark && writeSetupMark(mark, deps.storage);

function joinDone(form: JoinForm, res: JoinResponse | null, mark: SetupMark | null, deps: HomeDeps): JoinResult {
  form.inviteInput = "";
  if (res && res.waiting === true) return joinWaiting(form, mark, deps);
  form.status = joinedStatusText(quotedNet(res && res.label));
  writeMark(mark, deps);
  return "joined";
}

function joinWaiting(form: JoinForm, mark: SetupMark | null, deps: HomeDeps): JoinResult {
  form.status = JOIN_WAITING;
  writeMark(mark, deps);
  return "waiting";
}

async function sendJoin(form: JoinForm, body: JoinRequest, mark: SetupMark | null, deps: HomeDeps): Promise<JoinResult> {
  try {
    return joinDone(form, await request<JoinResponse>(form, "join", "/api/network/join", body, deps), mark, deps);
  } catch (err) {
    return joinFailed(form, err as ApiErrorLike);
  }
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
  return ask(form, async () => expectNode(form, store, deps, await startJoin(form, store, deps)));
}

function startJoin(form: JoinForm, store: Store, deps: HomeDeps): Promise<JoinResult> | "empty" {
  const key = form.inviteInput.trim();
  form.status = "";
  promiseTokenByForm.delete(form);
  stopLate(form);
  form.inviteError = key ? null : { message: INVITE_EMPTY, word: null };
  return key ? sendJoin(form, { key }, markFor(store), deps) : "empty";
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
  form.status = createdNotice((res && res.label) || label);
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

function startCreate(form: JoinForm, store: Store, deps: HomeDeps): Promise<JoinResult> | "empty" {
  const label = form.label.trim();
  form.labelError = label ? null : LABEL_EMPTY;
  return label ? sendCreate(form, label, { label }, markFor(store), deps) : "empty";
}
