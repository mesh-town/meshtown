/* Shared setup for the join-network tests (`join-actions.test.mjs`, `join-promise.test.mjs`): fake
 * `api`, `askKeep`, storage and timers for `joinNetwork`/`createNetwork` without Vue. */

import { createJoinForm } from "../home/join-actions.ts";
import { createStore } from "../core/store.ts";

export const pub = (ports) => ({ ports: { published: ports } });

export function fakeStorage(broken) {
  const data = new Map();
  const fail = () => { throw new Error("localStorage is unavailable"); };
  return {
    data,
    getItem: broken ? fail : (k) => (data.has(k) ? data.get(k) : null),
    setItem: broken ? fail : (k, v) => { data.set(k, String(v)); },
    removeItem: broken ? fail : (k) => { data.delete(k); },
  };
}

// Preconditions: published ports, the answer to "Keep publishing?", the node's reply or refusal;
// `hold` — the node's reply waits for `release()`; `boot` — `boot_id` in the node snapshot (the start the page sees on joining);
// deferred calls pile up in `timers`, the time comes by calling `fn()`.
export function setup({ ports = [], answer = "no", reply = {}, fail = null, hold = false, brokenStorage = false, inNet = false, noNotice = false, boot = "boot-1" } = {}) {
  const store = createStore();
  store.state = { boot_id: boot, network: inNet ? { id_hex: "net-old" } : null, ports: { published: ports.map((port) => ({ port })) } };
  store.config = pub(ports.map((port) => ({ port })));
  const calls = [];
  let release = () => {};
  const gate = new Promise((resolve) => { release = resolve; });
  const api = async (method, path, body) => {
    calls.push([method, path, structuredClone(body)]);
    if (hold) await gate;
    if (fail) throw fail;
    return reply;
  };
  let asks = 0;
  const askKeep = async () => { asks += 1; return answer; };
  const storage = fakeStorage(brokenStorage);
  const notices = [];
  const timers = [];
  const setTimer = (fn, ms) => timers.push({ fn, ms });
  const deps = { api, askKeep, storage, setTimer, notice: noNotice ? undefined : (text) => notices.push(text) };
  return { form: createJoinForm(), store, calls, storage, notices, timers, release, asked: () => asks, deps };
}
