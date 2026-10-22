/* Actions of the "Diagnostics" section over the form state `form` and the key `key`, without Vue:
 * the summary for help, copying the addresses, manually adding addresses, showing the network key.
 * `api`, the clipboard and the timer come via `deps`. The full key lives only in `key.full`. */

import { api as coreApi } from "../core/api.ts";
import type { ApiFn } from "../core/api.ts";
import type { LogResponse, NetworkKeyResponse, PeerAddressesAddResponse } from "../core/api-types.ts";
import type { Store } from "../core/store.ts";
import type { ClipboardDeps } from "../common/clipboard.ts";
import { writeClipboard } from "../common/clipboard.ts";
import { failText } from "../common/words.ts";
import {
  DIAG_ADDRESSES_COPIED, DIAG_ADDRESSES_FALLBACK, DIAG_CLIPBOARD_FALLBACK, DIAG_FAIL_ADD_ADDRESSES, DIAG_FAIL_GET_KEY, DIAG_KEY_MISSING,
  DIAG_KEY_STALE, DIAG_NO_ANSWER, DIAG_NO_LOG_DATA, DIAG_NO_NETWORK_KEY, DIAG_PEER_ADDR_EMPTY, DIAG_SUMMARY_COPIED,
} from "./diag-texts.ts";
import { diagnosticChecks } from "./diag-checks.ts";
import { peerAddrResultText } from "./diag-details.ts";
import { buildSummary, summaryLogPath } from "./diag-summary.ts";
import type { SummaryInput } from "./diag-summary.ts";

/** Dependencies of the section: `api`, the clipboard and the timer (replaced in tests). */
export interface DiagDeps extends ClipboardDeps {
  api: ApiFn;
  setTimer: (fn: () => void, ms: number) => unknown;
  clearTimer: (id: unknown) => void;
}

export interface DiagForm {
  specOpen: boolean;
  summary: { busy: boolean; status: string; fallback: string };
  peerAddr: { draft: string; busy: boolean; result: string };
  copyStatus: string;
}

export interface KeyState {
  shown: boolean;
  full: string | null;
  netId: string | null;
  busy: boolean;
  error: string;
}

export type DiagCopyResult = "copied" | "fallback" | "skipped";
export type PeerAddResult = "added" | "empty" | "failed" | "skipped";
export type KeyResult = "shown" | "hidden" | "stale" | "failed" | "skipped";

type TimerSlot = "summary" | "copy";

const STATUS_MS = 4000;

// We read `message` structurally: test fakes and foreign errors are not necessarily an `Error`.
const errMessage = (err: unknown): string => String((err as { message?: unknown } | null)?.message ?? "");

// Globals are taken at call time, not at import: tests run in node without a DOM.
export const defaultDiagDeps: DiagDeps = {
  api: coreApi,
  get clipboard() {
    return typeof navigator === "undefined" ? null : navigator.clipboard || null;
  },
  setTimer: (fn, ms) => setTimeout(fn, ms),
  clearTimer: (id) => clearTimeout(id as ReturnType<typeof setTimeout>),
};

export function createDiagForm(): DiagForm {
  return {
    specOpen: false,
    summary: { busy: false, status: "", fallback: "" },
    peerAddr: { draft: "", busy: false, result: "" },
    copyStatus: "",
  };
}

export function createKeyState(): KeyState {
  return { shown: false, full: null, netId: null, busy: false, error: "" };
}

// Timers that clear the statuses: the id is not put into the form, so that its shape matches the contract.
const timers = new WeakMap<DiagForm, Partial<Record<TimerSlot, unknown>>>();

function flash(form: DiagForm, slot: TimerSlot, set: (v: string) => void, text: string, deps: DiagDeps): void {
  const own = timers.get(form) || {};
  timers.set(form, own);
  deps.clearTimer(own[slot]);
  set(text);
  own[slot] = deps.setTimer(() => set(""), STATUS_MS);
}

export function clearDiagTimers(form: DiagForm, deps: DiagDeps = defaultDiagDeps): void {
  const own = timers.get(form) || {};
  Object.values(own).forEach((id) => deps.clearTimer(id));
  timers.delete(form);
}

async function fetchLogLines(path: string | null, deps: DiagDeps): Promise<Pick<SummaryInput, "lines" | "logError">> {
  if (!path) return { logError: DIAG_NO_LOG_DATA };
  try {
    const res = await deps.api<LogResponse>("GET", path);
    return { lines: res.lines || [] };
  } catch (err) {
    return { logError: errMessage(err) || DIAG_NO_ANSWER };
  }
}

function finishSummary(form: DiagForm, text: string, copied: boolean, deps: DiagDeps): DiagCopyResult {
  const sum = form.summary;
  sum.fallback = copied ? "" : text;
  const status = copied ? DIAG_SUMMARY_COPIED : DIAG_CLIPBOARD_FALLBACK;
  flash(form, "summary", (v) => { sum.status = v; }, status, deps);
  return copied ? "copied" : "fallback";
}

export async function copySummary(form: DiagForm, store: Store, deps: DiagDeps = defaultDiagDeps): Promise<DiagCopyResult> {
  const sum = form.summary;
  if (sum.busy) return "skipped";
  sum.busy = true;
  sum.fallback = "";
  try {
    const journal = await fetchLogLines(summaryLogPath(store.state), deps);
    const state = store.state;
    const text = buildSummary({ state, checks: diagnosticChecks(state), ...journal });
    return finishSummary(form, text, await writeClipboard(deps, text), deps);
  } finally {
    sum.busy = false;
  }
}

export async function copySelfAddresses(form: DiagForm, store: Store, deps: DiagDeps = defaultDiagDeps): Promise<DiagCopyResult> {
  const transport = store.state && store.state.transport;
  const text = ((transport && transport.self_addresses) || []).join("\n");
  const copied = await writeClipboard(deps, text);
  const status = copied ? DIAG_ADDRESSES_COPIED : DIAG_ADDRESSES_FALLBACK;
  flash(form, "copy", (v) => { form.copyStatus = v; }, status, deps);
  return copied ? "copied" : "fallback";
}

function clearDraftIf(pa: DiagForm["peerAddr"], text: string): void {
  if (pa.draft !== text) return;
  pa.draft = "";
}

function finishPeerAdd(pa: DiagForm["peerAddr"], text: string, res: PeerAddressesAddResponse): PeerAddResult {
  pa.result = peerAddrResultText(res);
  clearDraftIf(pa, text);
  return "added";
}

export async function addPeerAddresses(form: DiagForm, deps: DiagDeps = defaultDiagDeps): Promise<PeerAddResult> {
  const pa = form.peerAddr;
  if (pa.busy) return "skipped";
  const text = pa.draft;
  if (!text.trim()) {
    pa.result = DIAG_PEER_ADDR_EMPTY;
    return "empty";
  }
  pa.busy = true;
  try {
    const res = await deps.api<PeerAddressesAddResponse>("POST", "/api/peer-addresses", { text });
    return finishPeerAdd(pa, text, res);
  } catch (err) {
    pa.result = failText(DIAG_FAIL_ADD_ADDRESSES, err);
    return "failed";
  } finally {
    pa.busy = false;
  }
}

function hideKey(key: KeyState): void {
  key.shown = false;
  key.full = null;
  key.netId = null;
  key.error = "";
}

// The response is applied only if the network in the current snapshot is the same as at request time.
function applyKeyResponse(key: KeyState, store: Store, netId: string, res: NetworkKeyResponse | null): KeyResult {
  const current = store.state && store.state.network;
  if (!current || current.id_hex !== netId) {
    key.error = DIAG_KEY_STALE;
    return "stale";
  }
  if (!res || res.key == null) {
    key.error = DIAG_KEY_MISSING;
    return "failed";
  }
  key.full = res.key;
  key.netId = netId;
  key.shown = true;
  return "shown";
}

export async function toggleKey(key: KeyState, store: Store, deps: DiagDeps = defaultDiagDeps): Promise<KeyResult> {
  if (key.busy) return "skipped";
  if (key.shown) {
    hideKey(key);
    return "hidden";
  }
  const net = store.state && store.state.network;
  if (!net) return noNetwork(key);
  return requestKey(key, store, net.id_hex, deps);
}

function noNetwork(key: KeyState): KeyResult {
  key.error = DIAG_NO_NETWORK_KEY;
  return "failed";
}

async function requestKey(key: KeyState, store: Store, netId: string, deps: DiagDeps): Promise<KeyResult> {
  key.error = "";
  key.busy = true;
  try {
    return applyKeyResponse(key, store, netId, await deps.api<NetworkKeyResponse | null>("GET", "/api/network/key"));
  } catch (err) {
    key.error = failText(DIAG_FAIL_GET_KEY, err);
    return "failed";
  } finally {
    key.busy = false;
  }
}

export function resetKeyIfNetChanged(key: KeyState, netId: string | null): void {
  if (netId != null && netId === key.netId) return;
  hideKey(key);
}
