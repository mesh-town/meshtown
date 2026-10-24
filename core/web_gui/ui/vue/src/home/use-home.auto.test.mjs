/* `useHome` in the phone app: the exit is picked once on its own, never on a computer. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { nextTick } from "vue";
import { withSetup } from "../testing/with-setup.mjs";
import { createStore } from "../core/store.ts";
import { AUTO_PICKED_KEY, REMEMBERED_KEY } from "../common/exits.ts";
import { NET_B } from "../testing/overview-fixtures.mjs";
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

  it("a remembered id that is still offered is left alone", async () => {
    const w = setup({ peers: [peer("b", 90)] }, { data: { [REMEMBERED_KEY]: JSON.stringify([{ id: "b".repeat(64), slot: "main" }]) } });
    await nextTick();
    assert.equal(w.puts.length, 0);
    w.unmount();
  });

  it("a stale remembered id does not block the automatic pick", async () => {
    const w = setup({ peers: [peer("b", 90)] }, { data: { [REMEMBERED_KEY]: JSON.stringify([{ id: "x".repeat(64), slot: "main" }]) } });
    await nextTick();
    assert.equal(w.puts.length, 1);
    assert.deepEqual(w.puts[0].body.config.route.exits, ["b".repeat(64)]);
    w.unmount();
  });

  it("memory of another network does not block", async () => {
    const data = { [REMEMBERED_KEY]: JSON.stringify([{ id: "b".repeat(64), slot: NET_B }]) };
    const w = setup({ peers: [peer("b", 90)] }, { data, nets: true });
    await nextTick();
    assert.equal(w.puts.length, 1);
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

const brief = (id) => ({
  id, label: id, id_hex: "hex-" + id, status: "running", exit_enabled: false, proxy: id === "main", route_state: "direct", exits: [],
  members_voting: 1, joining: false, members_admitting: 0, me_excluded: false, me_pending: false, peers_online: 1, peers_total: 1,
});

// Two networks, the proxy in the primary one; network B has the same state.
function addNets(store) {
  store.nets = ["main", NET_B].map(brief);
  for (const b of store.nets) store.slots[b.id] = { id: b.id, brief: b, state: store.state, members: null, config: store.config, configRev: 1, configMeta: null, bootId: null };
}

function setup(state, { userAgent = APP_UA, exits = [], data = {}, nets = false } = {}) {
  const store = createStore();
  store.state = { node: { status: "online", id: "aaaa1111" }, network: { label: "home", id_hex: "n0" }, ...state };
  store.config = { node: { name: "n" }, route: { exits }, exit: { enabled: false } };
  store.configRev = 1;
  if (nets) addNets(store);
  const puts = [];
  const api = async (method, path, body) => { puts.push({ method, path, body }); return { rev: 2, config: body?.config ?? store.config }; };
  const storage = { getItem: (k) => data[k] ?? null, setItem: (k, v) => { data[k] = v; } };
  const deps = { api, clipboard: null, setTimer: () => 1, clearTimer: () => {}, storage, userAgent, focus: () => {}};
  const { result: h, unmount } = withSetup(() => useHome(store, deps));
  return { h, store, unmount, puts, data };
}
