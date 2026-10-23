/* Actions of the "Settings" section over the `form` state (save, restart,
 * stop, token). Without Vue: `api`, `confirm`, the timer, `busTarget` — via `deps`. */

import { CONFIRM_STOP_TEXT } from "../common/pac-texts.ts";
import { api as coreApi } from "../core/api.ts";
import type { ApiErrorLike, ApiFn } from "../core/api.ts";
import type { ChangedWhat } from "../core/bridge.ts";
import { notifyChanged } from "../core/bridge.ts";
import type { TokenRotateResponse } from "../core/api-types.ts";
import type { Store } from "../core/store.ts";
import { asResult, eachSlot, putConfigOnce, saveSlotFields, slotFailureText, targetSlots } from "../core/save-config.ts";
import { slotPath } from "../core/slots.ts";
import { getToken, setToken } from "../core/token.ts";
import { errReason, failText, upperFirst } from "../common/words.ts";
import { EXPERT_FIELDS, MAIN_FIELDS, PAGE_FIELDS, SHARED_PATHS, collectChanges } from "./settings-fields.ts";
import type { Field, RawValue } from "./settings-fields.ts";
import type { ConfigChanges, SlotFailure } from "../core/save-config.ts";
import { RESTART_STARTED, restartPending, restartTargets, savedMessageText, sharedChanges } from "./settings-view.ts";
import type { MessageView } from "./settings-view.ts";
import {
  SETTINGS_CONFIRM_ROTATE, SETTINGS_FAIL_RESTART, SETTINGS_FAIL_ROTATE, SETTINGS_FAIL_SAVE, SETTINGS_FAIL_STOP,
  SETTINGS_FIELD_ERRORS_MESSAGE, SETTINGS_PARTIAL, SETTINGS_SAVING, SETTINGS_TOKEN_ROTATED, SETTINGS_WRITE_FAILED,
} from "./settings-texts.ts";

/** Dependencies of the section (replaced in tests). */
export interface SettingsDeps {
  api: ApiFn;
  busTarget: EventTarget | undefined; // where to send `meshtown:changed`; no DOM — undefined
  confirm: (text: string) => boolean;
  setToken: (token: string) => void;
  getToken: () => string;
  setTimer: (fn: () => void, ms: number) => unknown;
  clearTimer: (id: unknown) => void;
}

export interface SettingsForm {
  draft: Record<string, RawValue>;
  errors: Record<string, string>;
  conflict: { rev: number | null } | null;
  message: MessageView;
  busy: boolean;
  restartRequested: boolean;
  hideFileFieldErrors: boolean;
  expertOpen: boolean;
  tokenRevealed: boolean;
  token: string;
  timer: unknown;
}

export type SaveOutcome = "saved" | "partial" | "invalid" | "conflict" | "failed" | "skipped";
export type RestartOutcome = "restarted" | "failed" | "skipped";

const ALL_FIELDS: readonly Field[] = [...MAIN_FIELDS, ...EXPERT_FIELDS, ...PAGE_FIELDS];
const SAVED_MS = 4000;
const CONFIRM_ROTATE = SETTINGS_CONFIRM_ROTATE;
const CONFIRM_STOP = CONFIRM_STOP_TEXT;

// Globals are taken at call time, not at import: tests run in node without a DOM.
export const defaultSettingsDeps: SettingsDeps = {
  api: coreApi,
  get busTarget() {
    return typeof document === "undefined" ? undefined : document;
  },
  confirm: (text: string) => confirm(text),
  setToken,
  getToken,
  setTimer: (fn, ms) => setTimeout(fn, ms),
  clearTimer: (id) => clearTimeout(id as ReturnType<typeof setTimeout>),
};

export function resolveSettingsDeps(deps: Partial<SettingsDeps>): SettingsDeps {
  return { ...defaultSettingsDeps, ...deps };
}

function notify(deps: SettingsDeps, what: ChangedWhat): void {
  if (!deps.busTarget) return;
  notifyChanged(deps.busTarget, what);
}

export function createSettingsForm(): SettingsForm {
  return {
    draft: {},
    errors: {},
    conflict: null,
    message: { text: "", cls: "hint" },
    busy: false,
    restartRequested: false,
    hideFileFieldErrors: false,
    expertOpen: false,
    tokenRevealed: false,
    token: "",
    timer: null,
  };
}

export function editField(form: SettingsForm, path: string, raw: RawValue): void {
  form.draft[path] = raw;
}

export function resetDraft(form: SettingsForm): void {
  form.draft = {};
  form.errors = {};
  form.conflict = null;
}

function say(form: SettingsForm, text: string, cls: string): void {
  form.message = { text, cls };
}

function clearStaleMessage(form: SettingsForm, store: Store, text: string): void {
  if (form.message.text !== text || restartPending(store.configMeta)) return;
  say(form, "", "hint");
}

function expireMessage(form: SettingsForm, store: Store, text: string): void {
  form.timer = null;
  clearStaleMessage(form, store, text);
}

function clearSavedLater(form: SettingsForm, store: Store, deps: SettingsDeps, text: string): void {
  deps.clearTimer(form.timer);
  form.timer = deps.setTimer(() => expireMessage(form, store, text), SAVED_MS);
}

// Clears the message timer when the section is unmounted.
export function disposeForm(form: SettingsForm, deps: SettingsDeps): void {
  if (form.timer === null) return;
  deps.clearTimer(form.timer);
  form.timer = null;
}

// Only what was sent and has not changed since leaves the draft: input typed during the PUT stays.
function keepEdited(draft: Record<string, RawValue>, sent: Record<string, RawValue>): Record<string, RawValue> {
  return Object.fromEntries(Object.entries(draft).filter(([path, raw]) => !(path in sent) || sent[path] !== raw));
}

// `sent` — a copy of the draft at the moment of sending.
function onSaved(form: SettingsForm, store: Store, deps: SettingsDeps, sent: Record<string, RawValue>): SaveOutcome {
  const text = savedMessageText(store.configMeta, Object.keys(sent));
  form.hideFileFieldErrors = true;
  form.errors = {};
  form.draft = keepEdited(form.draft, sent);
  form.restartRequested = false;
  say(form, text, "saved");
  clearSavedLater(form, store, deps, text);
  return "saved";
}

function onInvalid(form: SettingsForm, error: ApiErrorLike): SaveOutcome {
  const fields = (error.body && error.body.fields) || [];
  form.errors = Object.fromEntries(fields.map((f) => [f.path, f.message]));
  form.expertOpen = form.expertOpen || fields.some((f) => EXPERT_FIELDS.some((e) => e.path === f.path));
  say(form, SETTINGS_FIELD_ERRORS_MESSAGE, "field-error-text");
  return "invalid";
}

function onConflict(form: SettingsForm, error: ApiErrorLike): SaveOutcome {
  form.conflict = { rev: (error.body && error.body.rev) ?? null };
  form.message = { ...form.message, text: "" };
  return "conflict";
}

function onFailed(form: SettingsForm, error: ApiErrorLike): SaveOutcome {
  const noText = !(error.body && error.body.message);
  const text = noText && error.code === "config_write_failed" ? SETTINGS_WRITE_FAILED : failText(SETTINGS_FAIL_SAVE, error);
  say(form, text, "field-error-text");
  return "failed";
}

function onSaveError(form: SettingsForm, error: ApiErrorLike): SaveOutcome {
  if (error.code === "stale_rev") return onConflict(form, error);
  if (error.code === "invalid" || error.code === "not_available") return onInvalid(form, error);
  return onFailed(form, error);
}

// The whole config, no auto-retry: 409 → conflict. `overrideRev` — the revision from the 409 body.
export async function saveDraft(form: SettingsForm, store: Store, deps: Partial<SettingsDeps>, overrideRev?: number | null): Promise<SaveOutcome> {
  if (form.busy || (store.configFile && store.configFile.writable === false)) return "skipped";
  const d = resolveSettingsDeps(deps);
  const rev = overrideRev ?? store.configRev;
  const sent = { ...form.draft };
  form.busy = true;
  say(form, SETTINGS_SAVING, "hint");
  form.conflict = null;
  const changes = collectChanges(sent, ALL_FIELDS);
  const res = await putConfigOnce(store, changes, d, rev);
  const failures = res.ok ? await writeShared(store, changes, d) : [];
  form.busy = false;
  if (!res.ok) return onSaveError(form, res.error);
  return failures.length > 0 ? onPartial(form, sent, failures) : onSaved(form, store, d, sent);
}

// The shared paths go to the other networks after the main one took them; a slot without a key is skipped.
async function writeShared(store: Store, changes: ConfigChanges, d: SettingsDeps): Promise<SlotFailure[]> {
  const shared = sharedChanges(changes);
  if (Object.keys(shared).length === 0) return [];
  const others = targetSlots(store).filter((s) => s !== "main");
  return eachSlot(store, others, (slot) => saveSlotFields(store, slot, shared, d));
}

// The main config is saved; the shared fields stay in the draft so another "Save" brings the networks in step.
function onPartial(form: SettingsForm, sent: Record<string, RawValue>, failures: SlotFailure[]): SaveOutcome {
  const rest = Object.fromEntries(Object.entries(form.draft).filter(([path, raw]) => SHARED_PATHS.includes(path) || !(path in sent) || sent[path] !== raw));
  form.draft = rest;
  form.errors = {};
  say(form, SETTINGS_PARTIAL + " " + failures.map(slotFailureText).join(" "), "field-error-text");
  return "partial";
}

// The fresh config is brought by the shell (`shell/use-shell.js`) on `meshtown:changed{config}`.
export function reloadFresh(form: SettingsForm, deps: Partial<SettingsDeps>): void {
  resetDraft(form);
  notify(resolveSettingsDeps(deps), "config");
}

function onRestartAsked(form: SettingsForm): RestartOutcome {
  form.restartRequested = true;
  say(form, RESTART_STARTED, "saved");
  return "restarted";
}

function onRestartFailed(form: SettingsForm, failures: SlotFailure[]): RestartOutcome {
  const single = failures.length === 1 && failures[0].slot === "main";
  const text = single ? failText(SETTINGS_FAIL_RESTART, failures[0].error) : failures.map(slotFailureText).join(" ");
  say(form, text, "field-error-text");
  return "failed";
}

// Restarts every network that waits for it, the main one first; all must accept for "restarted".
export async function restartNode(form: SettingsForm, store: Store, deps: Partial<SettingsDeps>): Promise<RestartOutcome> {
  if (form.busy) return "skipped";
  const d = resolveSettingsDeps(deps);
  form.busy = true;
  const failures = await eachSlot(store, restartTargets(store), (slot) => asResult(d.api("POST", slotPath(slot, "actions/restart"), {}).then(() => undefined)));
  form.busy = false;
  return failures.length === 0 ? onRestartAsked(form) : onRestartFailed(form, failures);
}

export function onPendingEmpty(form: SettingsForm): void {
  form.restartRequested = false;
  if (form.message.text === RESTART_STARTED) say(form, "", "hint");
}

// A declined confirmation is silent; an error is a line in `form.message`; the `stopped` word sets the strip in
// the shell (`shell/use-shell.js`). `stopping` goes before the request so polling pauses, `state` after a
// failed request so polling resumes.
export async function stopNode(form: SettingsForm, deps: Partial<SettingsDeps>): Promise<boolean> {
  const d = resolveSettingsDeps(deps);
  if (!d.confirm(CONFIRM_STOP)) return false;
  notify(d, "stopping");
  try {
    await d.api("POST", "/api/actions/stop", {});
  } catch (err) {
    say(form, failText(SETTINGS_FAIL_STOP, err), "field-error-text");
    notify(d, "state");
    return false;
  }
  notify(d, "stopped");
  return true;
}

export async function rotateToken(form: SettingsForm, deps: Partial<SettingsDeps>): Promise<boolean> {
  const d = resolveSettingsDeps(deps);
  if (!d.confirm(CONFIRM_ROTATE)) return false;
  try {
    const res = await d.api<TokenRotateResponse | null>("POST", "/api/token/rotate", {});
    return applyRotated(form, d, res);
  } catch (err) {
    return rotateFailed(form, err); // the token stays the same
  }
}

// The node's message already starts with "token not changed: …" — only the capital letter is needed.
function rotateFailed(form: SettingsForm, err: unknown): boolean {
  const reason = errReason(err);
  say(form, reason ? upperFirst(reason) : failText(SETTINGS_FAIL_ROTATE, null), "field-error-text");
  return false;
}

function applyRotated(form: SettingsForm, deps: SettingsDeps, res: TokenRotateResponse | null): boolean {
  if (!res || !res.token) return rotateFailed(form, null);
  deps.setToken(res.token);
  form.token = res.token;
  form.tokenRevealed = true;
  say(form, SETTINGS_TOKEN_ROTATED, "saved");
  return true;
}

export function toggleToken(form: SettingsForm, deps: Partial<SettingsDeps>): void {
  const d = resolveSettingsDeps(deps);
  form.tokenRevealed = !form.tokenRevealed;
  form.token = form.tokenRevealed ? d.getToken() : "";
}
