/* `useHome` in the phone app: the exit is picked once on its own, never on a computer. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { nextTick } from "vue";
import { withSetup } from "../testing/with-setup.mjs";
import { createStore } from "../core/store.ts";
import { AUTO_PICKED_KEY, REMEMBERED_KEY } from "../common/exits.ts";
import { useHome } from "./use-home.ts";

const APP_UA = "Mozilla/5.0 (Linux; Android 14) Chrome/120 Mobile MeshTownApp/Android";
const DESKTOP_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) Chrome/126.0.0.0 Safari/537.36";
const peer = (ch, rtt, online = true) => ({ id: ch.repeat(10), id_full: ch.repeat(64), name: ch, online, exit: true, rtt_ms: rtt });

describe("useHome > automatic exit", () => {
  it("in the app: picks the nearest online exit and marks it done", async () => {
    const w = setup({ peers: [peer("b", 90), peer("c", 30), peer("d", 5, false)] });
    await nextTick();
    assert.equal(w.puts.length, 1);
    assert.deepEqual(w.puts[0].body.config.route.exits, ["c".repeat(64)]);
    assert.equal(w.data[AUTO_PICKED_KEY], "1");
    w.unmount();
  });

  it("picks once: the second change of the list does not pick again", async () => {
    const w = setup({ peers: [peer("b", 90)] });
    await nextTick();
    w.store.state = { ...w.store.state, peers: [...w.store.state.peers, peer("c", 1)] };
    await nextTick();
    assert.equal(w.puts.length, 1);
    w.unmount();
  });

  it("an exit that was chosen before (remembered) is left alone", async () => {
    const w = setup({ peers: [peer("b", 90)] }, { data: { [REMEMBERED_KEY]: JSON.stringify(["x".repeat(64)]) } });
    await nextTick();
    assert.equal(w.puts.length, 0);
    w.unmount();
  });

  it("an exit already in the config is left alone", async () => {
    const w = setup({ peers: [peer("b", 90)] }, { exits: ["b".repeat(64)] });
    await nextTick();
    assert.equal(w.puts.length, 0);
    w.unmount();
  });

  it("no online exit yet — waits, then picks when one appears", async () => {
    const w = setup({ peers: [peer("b", 90, false)] });
    await nextTick();
    assert.equal(w.puts.length, 0);
    w.store.state = { ...w.store.state, peers: [peer("b", 40)] };
    await nextTick();
    assert.equal(w.puts.length, 1);
    w.unmount();
  });

  it("on a computer nothing is picked", async () => {
    const w = setup({ peers: [peer("b", 90)] }, { userAgent: DESKTOP_UA });
    await nextTick();
    assert.equal(w.puts.length, 0);
    w.unmount();
  });
});

function setup(state, { userAgent = APP_UA, exits = [], data = {} } = {}) {
  const store = createStore();
  store.state = { node: { status: "online", id: "aaaa1111" }, network: { label: "home", id_hex: "n0" }, ...state };
  store.config = { node: { name: "n" }, route: { exits }, exit: { enabled: false } };
  store.configRev = 1;
  const puts = [];
  const api = async (method, path, body) => { puts.push({ method, path, body }); return { rev: 2, config: body?.config ?? store.config }; };
  const storage = { getItem: (k) => data[k] ?? null, setItem: (k, v) => { data[k] = v; } };
  const deps = { api, clipboard: null, setTimer: () => 1, clearTimer: () => {}, storage, userAgent, focus: () => {}, askKeep: async () => "no" };
  const { result: h, unmount } = withSetup(() => useHome(store, deps));
  return { h, store, unmount, puts, data };
}
