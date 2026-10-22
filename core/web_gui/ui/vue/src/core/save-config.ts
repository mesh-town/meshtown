/* Saving a config edit from a section (a port of `js/15-save-field.js::saveConfigFields`). */

import { toRaw } from "vue";
import type { ConfigResponse } from "./api-types.ts";
import { api as coreApi, ApiError, API_ERROR_TEXT } from "./api.ts";
import type { ApiErrorLike, ApiFn } from "./api.ts";
import { notifyChanged } from "./bridge.ts";
import { errReason } from "../common/words.ts";
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
  store.configMeta = {
    read_only: res.read_only || [],
    restart_fields: res.restart_fields || [],
    restart_pending: res.restart_pending || [],
    cli_overrides: res.cli_overrides || [],
  };
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
