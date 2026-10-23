/* Shared fixtures and helpers for the Settings form action tests. */

import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { EV_CHANGED } from "../core/bridge.ts";
import { createStore } from "../core/store.ts";
import { createSettingsForm } from "./settings-actions.ts";

export function meta(over = {}) {
  return { read_only: [], restart_fields: [], restart_pending: [], cli_overrides: [], ...over };
}

export function configBody() {
  return {
    socks: { enabled: true, port: 1080 },
    checks: { enabled: true, provider_dns: null },
    autostart: { available: true, reason: "", enabled: false },
  };
}

// The server's response to PUT: the config with the port edit applied, rev 6.
export function replyBody(over = {}) {
  const config = configBody();
  config.socks.port = 1081;
  return { config, rev: 6, ...over };
}

export function put200(over = {}) {
  return () => replyBody(over);
}

export function fail(code, status, body) {
  return () => {
    throw new ApiError(status, code, (body && body.message) || code, body);
  };
}

/** `script` — one response per `api` call; `confirm` — what the confirmation dialog answers. */
// A store whose overview lists the main network as waiting for a restart.
export function restartable(store) {
  store.restartSlots = ["main"];
  return store;
}

export function setup({ script, confirm = true }) {
  const store = createStore();
  store.config = configBody();
  store.configRev = 5;
  store.configMeta = meta();
  store.configFile = null;
  const form = createSettingsForm();
  const target = new EventTarget();
  const events = [];
  target.addEventListener(EV_CHANGED, (ev) => events.push(ev.detail));
  const calls = [];
  const api = async (method, path, body) => {
    calls.push({ method, path, body: body === undefined ? undefined : structuredClone(body) });
    const step = script[calls.length - 1];
    assert.ok(step, `unexpected request ${method} ${path}`);
    return step();
  };
  const timers = [];
  const tokens = [];
  const confirms = [];
  const deps = {
    api,
    busTarget: target,
    confirm: (text) => {
      confirms.push(text);
      return confirm;
    },
    setToken: (t) => tokens.push(t),
    setTimer: (fn, ms) => timers.push({ fn, ms }) && timers.length,
    clearTimer: () => {},
  };
  return { store, form, deps, calls, events, timers, tokens, confirms, target };
}

export function deferred() {
  let resolve;
  const promise = new Promise((r) => (resolve = r));
  return { promise, resolve };
}

export const tick = () => new Promise((r) => setImmediate(r));

export function plain(value) {
  return JSON.parse(JSON.stringify(value));
}
