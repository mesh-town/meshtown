/* Saving a config edit from a section (a port of `js/15-save-field.js::saveConfigFields`). */

import { toRaw } from "vue";
import type { ConfigResponse } from "./api-types.ts";
import { api as coreApi, ApiError, API_ERROR_TEXT } from "./api.ts";
import type { ApiErrorLike, ApiFn } from "./api.ts";
import { notifyChanged } from "./bridge.ts";
import { errReason } from "../common/words.ts";
import { configMeta, storeSlotConfig } from "./config-meta.ts";
import { netName, slotPath, visibleSlots } from "./slots.ts";
import { CORE_SAVE_FAILED_RETRY, coreSaveFailedText } from "./core-texts.ts";
import type { Store } from "./store.ts";

/** `api` is a (method, path, body) function, `busTarget` is where to send `meshtown:changed`. */
export interface SaveDeps {
  api?: ApiFn;
  busTarget?: EventTarget;
}

/** An edit: a dotted field path → the new value. */
export type ConfigChanges = Record<string, unknown>;

export type SaveResult = { ok: true } | { ok: false; error: ApiErrorLike };

type ResolvedDeps = { api: ApiFn; busTarget?: EventTarget };

// Dotted path; missing nodes are created.
export function setPath(obj: object, path: string, value: unknown): void {
  const parts = path.split(".");
  let cur = obj as Record<string, any>; // any: walking the document by string keys
  for (const key of parts.slice(0, -1)) {
    if (cur[key] === null || typeof cur[key] !== "object") cur[key] = {};
    cur = cur[key];
  }
  cur[parts[parts.length - 1]] = value;
}

// The PUT and GET /api/config responses are put into the store the same way.
function storeConfigReply(store: Store, res: ConfigResponse): void {
  store.config = res.config;
  store.configRev = res.rev;
  store.configMeta = configMeta(res);
  store.configFile = res.file || null;
}

// Edit values may arrive as a Vue reactive proxy (`ref([...]).value`): `structuredClone`
// fails on a proxy, so proxies are unwrapped at every depth.
function deepRaw(value: unknown): unknown {
  const raw = toRaw(value);
  if (Array.isArray(raw)) return raw.map(deepRaw);
  if (raw === null || typeof raw !== "object") return raw;
  return Object.fromEntries(Object.entries(raw).map(([k, v]) => [k, deepRaw(v)]));
}

async function putChanges(store: Store, changes: ConfigChanges, deps: ResolvedDeps, rev: number | null): Promise<void> {
  if (!store.config) throw new ApiError(0, "no_config", API_ERROR_TEXT.no_config);
  const config = structuredClone(toRaw(store.config));
  for (const [path, value] of Object.entries(changes)) setPath(config, path, deepRaw(value));
  const res = await deps.api<ConfigResponse>("PUT", "/api/config", { rev, config });
  storeConfigReply(store, res);
}

async function refetchConfig(store: Store, deps: ResolvedDeps): Promise<void> {
  const res = await deps.api<ConfigResponse | null>("GET", "/api/config");
  if (res) storeConfigReply(store, res);
}

async function putWithRetry(store: Store, changes: ConfigChanges, deps: ResolvedDeps): Promise<void> {
  try {
    await putChanges(store, changes, deps, store.configRev);
  } catch (err) {
    if ((err as ApiErrorLike).code !== "stale_rev") throw err;
    await refetchConfig(store, deps);
    await putChanges(store, changes, deps, store.configRev);
  }
}

const defaultSaveDeps: ResolvedDeps = { api: coreApi };

// Common tail: no config → error; `save(deps)` throws → `{ ok: false, error }`; otherwise `meshtown:changed`.
async function runSave(store: Store, deps: SaveDeps, save: (d: ResolvedDeps) => Promise<void>): Promise<SaveResult> {
  if (!store.config) return { ok: false, error: new ApiError(0, "no_config", API_ERROR_TEXT.no_config) };
  const d: ResolvedDeps = { ...defaultSaveDeps, ...deps };
  const error = await save(d).then(() => null, (err: unknown) => err as ApiErrorLike);
  if (error) return { ok: false, error };
  const target = d.busTarget || (typeof document === "undefined" ? undefined : document);
  if (target) notifyChanged(target, "config");
  return { ok: true };
}

// `deps = { api, busTarget }`: api is a (method, path, body) function, busTarget is where to send `meshtown:changed`.
export function saveConfigFields(store: Store, changes: ConfigChanges, deps: SaveDeps = defaultSaveDeps): Promise<SaveResult> {
  return runSave(store, deps, (d) => putWithRetry(store, changes, d));
}

// A single PUT with no GET and no retry: the 409 is returned to the caller so that it can show the conflict.
// `rev` defaults to the store revision; "save over" passes the revision from the 409 body.
export function putConfigOnce(
  store: Store,
  changes: ConfigChanges,
  deps: SaveDeps = defaultSaveDeps,
  rev: number | null = store.configRev,
): Promise<SaveResult> {
  return runSave(store, deps, (d) => putChanges(store, changes, d, rev));
}

// 422 — the field text from the response body; anything else — "Couldn't save: <node text>".
export function fieldErrorText(error: ApiErrorLike, changes: ConfigChanges): string {
  const path = Object.keys(changes)[0];
  if (error.code === "invalid" || error.code === "not_available") {
    const field = ((error.body && error.body.fields) || []).find((f) => f.path === path);
    if (field) return field.message;
  }
  const text = errReason(error);
  return text ? coreSaveFailedText(text) : CORE_SAVE_FAILED_RETRY;
}

// Slots: the same edit for a network added next to the main one. Its config lives in its slice.

const noConfig = (): ApiErrorLike => new ApiError(0, "no_config", API_ERROR_TEXT.no_config);

// The reply of a slot config PUT/GET is stored with the boot the slot's state came from.
function storeSlotReply(store: Store, slot: string, res: ConfigResponse): void {
  const slice = store.slots[slot];
  storeSlotConfig(store, slot, res, slice?.state?.boot_id ?? slice?.bootId ?? null);
}

async function putSlotChanges(store: Store, slot: string, changes: ConfigChanges, api: ApiFn): Promise<void> {
  const slice = store.slots[slot];
  if (!slice?.config) throw noConfig();
  const config = structuredClone(toRaw(slice.config));
  for (const [path, value] of Object.entries(changes)) setPath(config, path, deepRaw(value));
  storeSlotReply(store, slot, await api<ConfigResponse>("PUT", slotPath(slot, "config"), { rev: slice.configRev, config }));
}

async function putSlotWithRetry(store: Store, slot: string, changes: ConfigChanges, api: ApiFn): Promise<void> {
  try {
    await putSlotChanges(store, slot, changes, api);
  } catch (err) {
    if ((err as ApiErrorLike).code !== "stale_rev") throw err;
    const res = await api<ConfigResponse | null>("GET", slotPath(slot, "config"));
    if (res) storeSlotReply(store, slot, res);
    await putSlotChanges(store, slot, changes, api);
  }
}

// Like `saveConfigFields`, for any slot; the bus gets "state" (not "config": that one reloads the main config).
export async function saveSlotFields(store: Store, slot: string, changes: ConfigChanges, deps: SaveDeps = defaultSaveDeps): Promise<SaveResult> {
  if (slot === "main") return saveConfigFields(store, changes, deps);
  if (!store.slots[slot]?.config) return { ok: false, error: noConfig() };
  const d: ResolvedDeps = { ...defaultSaveDeps, ...deps };
  const error = await putSlotWithRetry(store, slot, changes, d.api).then(() => null, (err: unknown) => err as ApiErrorLike);
  if (error) return { ok: false, error };
  const target = d.busTarget || (typeof document === "undefined" ? undefined : document);
  if (target) notifyChanged(target, "state");
  return { ok: true };
}

/** A write that failed in one network. */
export interface SlotFailure {
  slot: string;
  label: string;
  error: ApiErrorLike;
}

/** A promise of a write as a `SaveResult`: a rejection becomes `{ ok: false }`, no value means success. */
export function asResult(p: Promise<SaveResult | void>): Promise<SaveResult> {
  return p.then((r) => r ?? { ok: true }, (error: unknown) => ({ ok: false, error: error as ApiErrorLike }));
}

// One at a time, the main slot first; a failure in one network does not stop the others.
export async function eachSlot(store: Store, ids: string[], write: (slot: string) => Promise<SaveResult>): Promise<SlotFailure[]> {
  const ordered = [...ids.filter((id) => id === "main"), ...ids.filter((id) => id !== "main")];
  const failures: SlotFailure[] = [];
  for (const slot of ordered) {
    const res = await asResult(Promise.resolve().then(() => write(slot)));
    if (!res.ok) failures.push({ slot, label: netName(store.nets, slot), error: res.error });
  }
  return failures;
}

export const slotFailureText = (f: SlotFailure): string => `"${f.label}": ${errReason(f.error) || f.error.message}`;

// Visible slots that hold a network key and are not being joined, the main one first.
export function targetSlots(store: Store): string[] {
  const held = visibleSlots(store.nets, store.slots).filter((id) => {
    const brief = store.nets.find((n) => n.id === id);
    return brief != null && brief.id_hex != null && !brief.joining;
  });
  return [...held.filter((id) => id === "main"), ...held.filter((id) => id !== "main")];
}
