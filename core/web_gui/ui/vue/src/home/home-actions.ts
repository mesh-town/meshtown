/* The "Home" actions over the `form` form without Vue: the exit choice, "Check a site", the node name,
 * the "Allow others" switch, copying the proxy address, the "Three steps" setup mark.
 * `api`, the storage, the clipboard and the timer come through `deps`; after an `await` — only the current `store`/`form`. */

import { api as coreApi } from "../core/api.ts";
import type { ApiFn } from "../core/api.ts";
import type { SaveResult, ConfigChanges } from "../core/save-config.ts";
import type { Store } from "../core/store.ts";
import { PAC_COPY_FALLBACK } from "../common/pac-texts.ts";
import type { BrowserId, OsId, StepValue } from "./browser-setup.ts";
import type { ProxyScope } from "./proxy-scope.ts";
import { freshCursor } from "./wizard/wizard-machine.ts";
import type { WizardCursor } from "./wizard/wizard-machine.ts";
import type { WizBrowser } from "./wizard/wizard-steps.ts";
import { saveSlotFields, fieldErrorText } from "../core/save-config.ts";
import { exitChoiceValue, rememberCurrentExit, saveDirect, saveExitIn } from "../core/exit-via.ts";
import { parseExitValue } from "./exit-list.ts";
import { writeClipboard } from "../common/clipboard.ts";
import { failText } from "../common/words.ts";
import { addRememberedFor } from "../common/exits.ts";
import {
  HOME_EMPTY_FIND, HOME_FIND_ACTION, HOME_KIND_ADDR, HOME_KIND_LAN, HOME_KIND_PORT, HOME_SAVED, clipboardFallbackText,
} from "./home-flow-texts.ts";

const SETUP_KEY = "meshtown.homeSetup";

export type SetupMark = "browser" | "exit" | "done";
export interface FieldStatus {
  text: string;
  cls: string;
}
interface SetupStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

/** Everything external that "Home" needs: `api`, the clipboard, the timer, the storage, `userAgent`, focus, the notification. */
export interface HomeDeps {
  api: ApiFn;
  readonly clipboard: { writeText: (text: string) => Promise<void> } | null;
  setTimer: (fn: () => void, ms: number) => unknown;
  clearTimer: (id: unknown) => void;
  readonly storage: SetupStorage | null;
  readonly userAgent: string;
  focus: (selector: string) => void;
  /** Exists only on "Home" (put there by `use-join`). */
  notice?: (text: string) => void;
}

export interface HomeForm {
  exitChoice: string;
  exitSavesInFlight: number;
  exitStatus: FieldStatus;
  findDomain: string;
  findDraft: string;
  findError: string;
  name: { draft: string; original: string; focused: boolean; status: FieldStatus };
  exitServerStatus: FieldStatus;
  /** `scope`, `os` and `pick` are the wizard's answers (also read by the Main card); `choice` is the manual block's browser. */
  browser: {
    choice: BrowserId | null; os: OsId | null; scope: ProxyScope | null; pick: WizBrowser | null;
    open: boolean; copied: "" | StepValue; note: string; noteKind: "" | StepValue;
  };
  /** The wizard's cursor; it lives as long as the section, so a sub-tab switch keeps the place. */
  wizard: WizardCursor;
}

export type ExitSaveResult = "saved" | "failed";
const COPIED_MS = 2000;
const SAVED = { text: HOME_SAVED, cls: "saved" };

// Globals are taken at call time, not at import: the tests run in node without a DOM.
export const defaultHomeDeps: HomeDeps = {
  api: coreApi,
  get clipboard() {
    return typeof navigator === "undefined" ? null : navigator.clipboard || null;
  },
  setTimer: (fn, ms) => setTimeout(fn, ms),
  clearTimer: (id) => clearTimeout(id as ReturnType<typeof setTimeout>),
  get storage() {
    return typeof localStorage === "undefined" ? null : localStorage;
  },
  get userAgent() {
    return typeof navigator === "undefined" ? "" : navigator.userAgent || "";
  },
  focus: (selector) => {
    const el = typeof document === "undefined" ? null : document.querySelector<HTMLElement>(selector);
    if (el) el.focus();
  },
};

const noStatus = (): FieldStatus => ({ text: "", cls: "" });

export function createHomeForm(): HomeForm {
  return {
    exitChoice: "",
    exitSavesInFlight: 0,
    exitStatus: noStatus(),
    findDomain: "",
    findDraft: "",
    findError: "",
    name: { draft: "", original: "", focused: false, status: noStatus() },
    exitServerStatus: noStatus(),
    browser: { choice: null, os: null, scope: null, pick: null, open: false, copied: "", note: "", noteKind: "" },
    wizard: freshCursor(),
  };
}

// The setup mark: null, "browser", "exit", "done". Storage errors are swallowed.
export function readSetupMark(storage: SetupStorage | null): string | null {
  try {
    return storage!.getItem(SETUP_KEY); // storage: null falls into catch — "no storage"
  } catch (err) {
    return null;
  }
}

export function writeSetupMark(mark: string, storage: SetupStorage | null): void {
  try {
    storage!.setItem(SETUP_KEY, mark); // storage: null falls into catch
  } catch (err) {
    /* storage is unavailable — the setup won't show after joining */
  }
}

function resultStatus(res: SaveResult, changes: ConfigChanges): FieldStatus {
  return res.ok ? SAVED : { text: fieldErrorText(res.error, changes), cls: "field-error-text" };
}

// The `exitSavesInFlight` counter holds the store's watch back from overwriting `exitChoice` during our PUT.
async function exitSave(form: HomeForm, store: Store, value: string, save: () => Promise<FieldStatus>): Promise<ExitSaveResult> {
  form.exitSavesInFlight++;
  try {
    form.exitStatus = await save();
  } finally {
    form.exitSavesInFlight--;
  }
  const ok = form.exitStatus.cls === "saved";
  settleChoice(form, store, value, ok);
  return ok ? "saved" : "failed";
}

// While another PUT is in flight, we leave the choice to it: the last saved one will set `exitChoice` itself.
function settleChoice(form: HomeForm, store: Store, value: string, ok: boolean): void {
  if (form.exitSavesInFlight > 0) return;
  form.exitChoice = ok ? value : exitChoiceValue(store);
}

// `value` is the radio value (`exitValue`): the exit and the network it is taken in.
export function chooseExit(form: HomeForm, store: Store, value: string, deps: HomeDeps = defaultHomeDeps): Promise<ExitSaveResult> {
  const { slot, id } = parseExitValue(value);
  const remember = (exits: string[]) => addRememberedFor(slot, exits, storageOf(deps));
  return exitSave(form, store, value, () => saveExitIn(store, slot, id, { api: deps.api, remember }));
}

// No storage (null) — `setRememberedExits` swallows the error itself, so we pass null as is.
export function storageOf(deps: HomeDeps): SetupStorage {
  return deps.storage as SetupStorage;
}

// "As usual": the chosen exit stays a list row (remembered), the choice itself is cleared.
export function chooseDirect(form: HomeForm, store: Store, deps: HomeDeps = defaultHomeDeps): Promise<ExitSaveResult> {
  rememberCurrentExit(store, (slot, ids) => addRememberedFor(slot, ids, storageOf(deps)));
  return exitSave(form, store, "", () => saveDirect(store, { api: deps.api }));
}

const EMPTY_FIND = HOME_EMPTY_FIND;

// The domain from the `findDraft` field goes into the check; "busy" (409) doesn't cancel the wait, anything else is the error text.
export async function findExit(form: HomeForm, deps: HomeDeps = defaultHomeDeps): Promise<"empty" | "sent" | "failed"> {
  const domain = form.findDraft.trim();
  form.findError = "";
  if (!domain) {
    form.findError = EMPTY_FIND;
    return "empty";
  }
  form.findDomain = domain;
  form.findDraft = "";
  try {
    await deps.api("POST", "/api/actions/find-exit", { domain });
    return "sent";
  } catch (err) {
    return findRefused(form, err);
  }
}

function findRefused(form: HomeForm, err: unknown): "sent" | "failed" {
  if ((err as { status?: number } | null)?.status === 409) return "sent";
  form.findError = failText(HOME_FIND_ACTION, err);
  form.findDomain = "";
  return "failed";
}

async function persistName(form: HomeForm, store: Store, value: string, deps: HomeDeps): Promise<boolean> {
  const name = form.name;
  const changes = { "node.name": value };
  const res = await saveSlotFields(store, store.lead, changes, { api: deps.api });
  name.status = resultStatus(res, changes);
  return res.ok;
}

// Saves only if the name differs from the saved one; empty — an empty name is saved.
export async function saveName(form: HomeForm, store: Store, deps: HomeDeps = defaultHomeDeps): Promise<"same" | "saved" | "failed"> {
  const name = form.name;
  const value = name.draft.trim();
  if (value === name.original) return "same";
  const before = name.original;
  name.original = value;
  name.draft = value;
  const ok = await persistName(form, store, value, deps);
  name.original = ok ? value : before;
  return ok ? "saved" : "failed";
}

export async function saveExitServer(form: HomeForm, store: Store, checked: boolean, deps: HomeDeps = defaultHomeDeps): Promise<ExitSaveResult> {
  const changes = { "exit.enabled": checked };
  const res = await saveSlotFields(store, store.lead, changes, { api: deps.api });
  form.exitServerStatus = resultStatus(res, changes);
  return res.ok ? "saved" : "failed";
}

// Timers that clear "Copied": we don't put the id into the form so that its shape matches the contract.
const timers = new WeakMap<HomeForm, unknown>();

export function clearHomeTimers(form: HomeForm, deps: HomeDeps = defaultHomeDeps): void {
  deps.clearTimer(timers.get(form));
  timers.delete(form);
}

function flashCopied(form: HomeForm, kind: StepValue, deps: HomeDeps): void {
  clearHomeTimers(form, deps);
  form.browser.copied = kind;
  timers.set(form, deps.setTimer(() => { form.browser.copied = ""; }, COPIED_MS));
}

const KIND_WORD: Record<StepValue, string> = { host: HOME_KIND_ADDR, port: HOME_KIND_PORT, lan: HOME_KIND_LAN, pac: HOME_KIND_ADDR };

const fallbackNote = (kind: StepValue): string =>
  kind === "pac" ? PAC_COPY_FALLBACK : clipboardFallbackText(KIND_WORD[kind]);

// `kind` — "host" | "port" | "lan" | "pac": for 2000 ms `form.browser.copied` = kind ("Copied");
// no clipboard — `form.browser.note` asks to select the value and copy it manually.
export async function copyValue(form: HomeForm, kind: StepValue, value: string, deps: HomeDeps = defaultHomeDeps): Promise<"fallback" | "copied"> {
  form.browser.note = "";
  form.browser.noteKind = kind;
  if (!(await writeClipboard(deps, value))) {
    form.browser.note = fallbackNote(kind);
    return "fallback";
  }
  flashCopied(form, kind, deps);
  return "copied";
}
