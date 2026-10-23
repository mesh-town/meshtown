/* Shared harness of the shell tests: a stub `deps`, an event target with a listener count, and a host component
 * mounted in an empty renderer (without DOM). */

import { nextTick } from "vue";
import { ApiError } from "../core/api.ts";
import { createStore } from "../core/store.ts";
import { adoptUrlToken } from "./token-url.ts";
import { useShell } from "./use-shell.ts";
import { withSetup } from "../testing/with-setup.mjs";
import { NET_B } from "../testing/overview-fixtures.mjs";

const CONFIG_REPLY = { config: {}, rev: 1 };
export const STATE = { boot_id: "b1", config_rev: 1, node: { status: "running" }, network: null, peers: [] };

export const flush = () => new Promise((resolve) => setImmediate(resolve));
export const settle = async () => { await flush(); await flush(); await nextTick(); };

// The event target, taking the number of listeners into account.
function fakeTarget() {
  const map = new Map();
  return {
    addEventListener: (name, fn) => map.set(name, [...(map.get(name) || []), fn]),
    removeEventListener: (name, fn) => map.set(name, (map.get(name) || []).filter((f) => f !== fn)),
    fire: (name, ev = {}) => (map.get(name) || []).slice().forEach((fn) => fn(ev)),
    listeners: (name) => (map.get(name) || []).length,
  };
}

// Harness: stub `deps`, the host component is mounted in an empty renderer (without DOM).
export function build({ hash = "", href = "http://h/", overview = null } = {}) {
  const store = createStore();
  const doc = fakeTarget();
  const win = fakeTarget();
  const data = new Map();
  const storage = { getItem: (k) => data.get(k) ?? null, setItem: (k, v) => data.set(k, v) };
  const env = { store, doc, win, storage, calls: [], timers: [], scrolled: 0, shell: null, stateReply: null, overviewReply: overview ? () => overview(env) : null, replacedUrls: [], nets: ["main", NET_B] };
  env.deps = {
    api: async (method, path) => {
      env.calls.push(method + " " + path);
      if (path === "/api/overview" && env.overviewReply) return env.overviewReply();
      if (path === "/api/state") return env.stateReply ? env.stateReply() : STATE;
      if (path === "/api/config" || /^\/api\/net\/[a-z2-7]{6}\/config$/.test(path)) return CONFIG_REPLY;
      throw new ApiError(404, "not_found");
    },
    setTimer: (fn, ms) => (env.timers.push({ fn, ms, live: true }), env.timers.length),
    clearTimer: (id) => { if (id && env.timers[id - 1]) env.timers[id - 1].live = false; },
    isHidden: () => Boolean(env.deps.hiddenNow),
    replaceUrl: (url) => { env.replacedUrls.push(url); },
    scrollToTop: () => { env.scrolled++; },
    href,
    hash,
    storage,
    doc,
    win,
  };
  env.answerStateWith = (fail) => { env.stateReply = fail ? () => { throw fail; } : null; };
  env.active = () => env.timers.filter((t) => t.live);
  env.count = (key) => env.calls.filter((c) => c === key).length;
  env.bus = (what) => doc.fire("meshtown:changed", { detail: { what } });
  env.fireZeroTimers = async () => {
    for (const t of env.active().filter((x) => x.ms === 0)) {
      t.live = false;
      await t.fn();
    }
  };
  env.fireAllTimers = async () => {
    for (const t of env.active()) {
      t.live = false;
      await t.fn();
    }
  };
  adoptUrlToken(env.deps);
  const mounted = withSetup(() => useShell(store, env.deps));
  env.shell = mounted.result;
  env.unmount = mounted.unmount;
  return env;
}

export async function mountShell(opts = {}) {
  const env = build(opts);
  await flush();
  return env;
}
