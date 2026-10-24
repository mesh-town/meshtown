/* `exit-via.ts`: the status and remembering after "Exit via …", the exit of any network, "Directly", the proxy network of the summary. */

import { describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { REMEMBERED_KEY } from "../common/exits.ts";
import { NET_B } from "../testing/overview-fixtures.mjs";
import { ApiError } from "./api.ts";
import { exitChoiceValue, proxySlotOf, saveDirect, saveExitIn, saveSingleExit } from "./exit-via.ts";
import { createStore } from "./store.ts";

const NET_C = "m3n4pq";

function setup(fail = null) {
  const store = createStore();
  store.config = { route: { exits: [] } };
  store.configRev = 1;
  const calls = [];
  const remembered = [];
  const api = async (method, path, body) => {
    calls.push(structuredClone(body));
    if (fail) throw fail;
    return { config: body.config, rev: 2 };
  };
  return { store, calls, remembered, deps: { api, busTarget: new EventTarget(), remember: (x) => remembered.push(x) } };
}

describe("saveSingleExit", () => {
  it("success: PUT with the single exit, \"Saved\", the choice is remembered", async () => {
    const w = setup();
    const st = await saveSingleExit(w.store, "AA", w.deps);
    assert.deepEqual(st, { text: "Saved", cls: "saved" });
    assert.deepEqual(w.calls[0].config.route.exits, ["AA"]);
    assert.deepEqual(w.remembered, [["AA"]]);
  });

  it("refusal: the field error text, nothing is remembered", async () => {
    const w = setup(new ApiError(500, "boom", "failure"));
    const st = await saveSingleExit(w.store, "AA", w.deps);
    assert.equal(st.cls, "field-error-text");
    assert.match(st.text, /^Couldn't save: /);
    assert.deepEqual(w.remembered, []);
  });
});

describe("saveSingleExit in several networks", () => {
  it("the leading network is B: the exit is written into B, not into the main config", async () => {
    const w = setupNets({ proxy: "main", lead: NET_B });
    const st = await saveSingleExit(w.store, "BB", w.deps);
    assert.deepEqual(st, { text: "Saved", cls: "saved" });
    assert.deepEqual(w.paths(), [`PUT /api/net/${NET_B}/config`, "GET /api/config"]);
    assert.deepEqual(w.calls[0].body.config.route.exits, ["BB"]);
  });
});

describe("saveExitIn", () => {
  describe("an exit of B while the proxy points to A (main)", () => {
    it("PUT /api/net/<B>/config with the single exit, the main config is not written", async () => {
      const w = setupNets({ proxy: "main" });
      const st = await saveExitIn(w.store, NET_B, "BB", w.deps);
      assert.deepEqual(st, { text: "Saved", cls: "saved" });
      assert.deepEqual(w.paths(), [`PUT /api/net/${NET_B}/config`, "GET /api/config"]);
      assert.deepEqual(w.calls[0].body.config.route.exits, ["BB"]);
      assert.equal(w.calls[0].body.rev, 11, "the revision of B's slice, not the main one");
    });

    it("the choice is remembered as {id, slot: B}, the entries of the other networks stay", async () => {
      const w = setupNets({ proxy: "main", remembered: ["MM", { id: "CC", slot: NET_C }] });
      await saveExitIn(w.store, NET_B, "BB", w.deps);
      const entries = w.stored();
      assert.ok(entries.some((e) => e.id === "BB" && e.slot === NET_B), "B's exit is not remembered");
      assert.ok(entries.some((e) => e.id === "MM" && e.slot === "main"), "the main entry was lost");
      assert.ok(entries.some((e) => e.id === "CC" && e.slot === NET_C), "C's entry was lost");
    });

    it("a refusal: the field error text, nothing new is remembered", async () => {
      const w = setupNets({ proxy: "main", fail: new ApiError(500, "boom", "failure") });
      const st = await saveExitIn(w.store, NET_B, "BB", w.deps);
      assert.equal(st.cls, "field-error-text");
      assert.match(st.text, /^Couldn't save: /);
      assert.deepEqual(w.stored(), []);
    });
  });

  describe("an exit of the main network", () => {
    it("PUT /api/config with the single exit, as before", async () => {
      const w = setupNets({ proxy: NET_B });
      const st = await saveExitIn(w.store, "main", "AA", w.deps);
      assert.deepEqual(st, { text: "Saved", cls: "saved" });
      assert.deepEqual(w.paths(), ["PUT /api/config"]);
      assert.deepEqual(w.calls[0].body.config.route.exits, ["AA"]);
    });

    it("the choice is remembered in the main slot", async () => {
      const w = setupNets({ proxy: NET_B });
      await saveExitIn(w.store, "main", "AA", w.deps);
      assert.ok(w.stored().some((e) => e.id === "AA" && e.slot === "main"));
    });
  });

  describe("B already has exactly this exit but is not the proxy network", () => {
    it("PUT /api/config {proxy: {net: B}}, B's own config is not written", async () => {
      const w = setupNets({ proxy: "main", exits: { [NET_B]: ["BB"] } });
      const st = await saveExitIn(w.store, NET_B, "BB", w.deps);
      assert.deepEqual(st, { text: "Saved", cls: "saved" });
      assert.deepEqual(w.paths(), ["PUT /api/config", "GET /api/config"]);
      assert.equal(w.calls[0].body.config.proxy.net, NET_B);
    });
  });

  describe("one network", () => {
    it("PUT /api/config, the exit is remembered as an object of the main slot", async () => {
      const w = setupNets({ slots: ["main"] });
      await saveExitIn(w.store, "main", "AA", w.deps);
      assert.deepEqual(w.paths(), ["PUT /api/config"]);
      assert.deepEqual(w.calls[0].body.config.route.exits, ["AA"]);
      assert.equal(w.storage.getItem(REMEMBERED_KEY), JSON.stringify([{ id: "AA", slot: "main" }]));
    });
  });
});

describe("saveDirect", () => {
  describe("the proxy points to the main network", () => {
    it("PUT /api/config with empty route.exits and no change of proxy.net", async () => {
      const w = setupNets({ proxy: "main", exits: { main: ["AA"] }, mainConfig: { route: { exits: ["AA"] } } });
      const st = await saveDirect(w.store, w.deps);
      assert.deepEqual(st, { text: "Saved", cls: "saved" });
      assert.deepEqual(w.paths(), ["PUT /api/config"]);
      assert.deepEqual(w.calls[0].body.config, { route: { exits: [] } });
    });
  });

  describe("the proxy points to B", () => {
    it("one PUT /api/config with both fields: route.exits is empty and proxy.net is main", async () => {
      const w = setupNets({ proxy: NET_B, exits: { [NET_B]: ["BB"] }, mainConfig: { route: { exits: ["AA"] }, proxy: { net: NET_B } } });
      await saveDirect(w.store, w.deps);
      assert.deepEqual(w.paths(), ["PUT /api/config"]);
      assert.deepEqual(w.calls[0].body.config.route.exits, []);
      assert.equal(w.calls[0].body.config.proxy.net, "main");
    });

    it("B's own config is not written", async () => {
      const w = setupNets({ proxy: NET_B, exits: { [NET_B]: ["BB"] } });
      await saveDirect(w.store, w.deps);
      assert.ok(!w.paths().some((p) => p.includes(`/api/net/${NET_B}/`)), w.paths().join(", "));
    });
  });

  describe("no network has the proxy flag", () => {
    it("both fields in one PUT /api/config, same as for another network", async () => {
      const w = setupNets({ proxy: null, mainConfig: { route: { exits: ["AA"] }, proxy: { net: "zzzzzz" } } });
      await saveDirect(w.store, w.deps);
      assert.deepEqual(w.paths(), ["PUT /api/config"]);
      assert.deepEqual(w.calls[0].body.config.route.exits, []);
      assert.equal(w.calls[0].body.config.proxy.net, "main");
    });
  });

  describe("a refusal", () => {
    it("the field error text", async () => {
      const w = setupNets({ proxy: NET_B, fail: new ApiError(500, "boom", "failure") });
      const st = await saveDirect(w.store, w.deps);
      assert.equal(st.cls, "field-error-text");
      assert.match(st.text, /^Couldn't save: /);
    });
  });
});

describe("proxySlotOf", () => {
  it("the proxy flag is on B: B", () => {
    assert.equal(proxySlotOf(setupNets({ proxy: NET_B }).store), NET_B);
  });

  it("the proxy flag is on the main network: main", () => {
    assert.equal(proxySlotOf(setupNets({ proxy: "main" }).store), "main");
  });

  it("no slot has the flag (the proxy points to a network this device is not in): null", () => {
    assert.equal(proxySlotOf(setupNets({ proxy: null }).store), null);
  });

  it("the flag is on a slot that is not visible (no key, no state): null", () => {
    const w = setupNets({ proxy: NET_B });
    hide(w.store, NET_B);
    assert.equal(proxySlotOf(w.store), null);
  });

  it("one network with the flag: main", () => {
    assert.equal(proxySlotOf(setupNets({ slots: ["main"] }).store), "main");
  });
});

describe("exitChoiceValue", () => {
  describe("several networks", () => {
    it("the proxy is in A: the first exit of A as \"<slot>|<id>\"", () => {
      const w = setupNets({ proxy: "main", exits: { main: ["AA", "A2"], [NET_B]: ["BB"] } });
      assert.equal(exitChoiceValue(w.store), "main|AA");
    });

    it("the proxy is in B: the exit of B, with B's slot", () => {
      const w = setupNets({ proxy: NET_B, exits: { main: ["AA"], [NET_B]: ["BB"] } });
      assert.equal(exitChoiceValue(w.store), `${NET_B}|BB`);
    });

    it("three networks, the proxy is in C: the exit of C", () => {
      const w = setupNets({ slots: ["main", NET_B, NET_C], proxy: NET_C, exits: { main: ["AA"], [NET_B]: ["BB"], [NET_C]: ["CC"] } });
      assert.equal(exitChoiceValue(w.store), `${NET_C}|CC`);
    });

    it("PUT into B answered 200 but the summary still has the proxy in A: the value is A's exit, not B's", async () => {
      const w = setupNets({ proxy: "main", exits: { main: ["AA"] } });
      await saveExitIn(w.store, NET_B, "BB", w.deps);
      assert.equal(exitChoiceValue(w.store), "main|AA");
    });

    it("PUT into B answered 200 and the summary has no exit in A: empty, not B's exit", async () => {
      const w = setupNets({ proxy: "main" });
      await saveExitIn(w.store, NET_B, "BB", w.deps);
      assert.equal(exitChoiceValue(w.store), "");
    });

    it("the proxy network has no exit (direct): empty", () => {
      assert.equal(exitChoiceValue(setupNets({ proxy: NET_B, exits: { main: ["AA"] } }).store), "");
    });

    it("no slot has the proxy flag: empty", () => {
      assert.equal(exitChoiceValue(setupNets({ proxy: null, exits: { main: ["AA"], [NET_B]: ["BB"] } }).store), "");
    });
  });

  describe("one network", () => {
    it("the value is the bare id, as before", () => {
      assert.equal(exitChoiceValue(setupNets({ slots: ["main"], exits: { main: ["AA"] } }).store), "AA");
    });

    it("no exit: empty", () => {
      assert.equal(exitChoiceValue(setupNets({ slots: ["main"] }).store), "");
    });
  });
});

// A store with the listed networks (all hold a key and have a state). `proxy` — the slot with `brief.proxy` (`null` — none),
// `exits` — slot → full ids of its exits (the summary and the state agree), `lead` — the leading slot, `mainConfig` — the main
// config document, `remembered` — what the page has already remembered (ids of the main slot or {id, slot}), `fail` — an
// error for every request. Nothing is mocked but the network: the deps carry a request log and an in-memory localStorage.
function setupNets({ slots = ["main", NET_B], proxy = "main", exits = {}, lead = "main", mainConfig = null, remembered = null, fail = null } = {}) {
  const storage = memoryStorage();
  if (remembered) storage.setItem(REMEMBERED_KEY, JSON.stringify(remembered));
  vi.stubGlobal("localStorage", storage);
  const store = createStore();
  store.config = mainConfig || { route: { exits: [] } };
  store.configRev = 5;
  store.nets = slots.map((id) => brief(id, { proxy: id === proxy, exits: exits[id] || [] }));
  for (const b of store.nets) store.slots[b.id] = slice(b);
  store.lead = lead;
  store.state = store.slots[lead].state;
  const calls = [];
  const api = async (method, path, body) => {
    calls.push({ method, path, body: structuredClone(body) });
    if (fail) throw fail;
    if (method === "GET") return { config: { route: { exits: [] } }, rev: 5 };
    return { config: body.config, rev: body.rev + 1 };
  };
  const deps = { api, busTarget: new EventTarget() };
  const paths = () => calls.map((c) => `${c.method} ${c.path}`);
  const stored = () => (storage.getItem(REMEMBERED_KEY) ? JSON.parse(storage.getItem(REMEMBERED_KEY)).map(asEntry) : []);
  return { store, calls, deps, storage, paths, stored };
}

const asEntry = (item) => (typeof item === "string" ? { id: item, slot: "main" } : item);

function brief(id, over = {}) {
  return {
    id, label: id === "main" ? "friends" : "net " + id, id_hex: "hex-" + id, status: "running", exit_enabled: false, proxy: false,
    route_state: "direct", exits: [], members_voting: 1, joining: false, members_admitting: 0,
    me_excluded: false, me_pending: false, peers_online: 0, peers_total: 0, ...over,
  };
}

function slice(b) {
  const route = { state: b.exits.length ? "exit" : "direct", exits: b.exits.map((id) => routeExit(id)) };
  const state = { boot_id: "boot-" + b.id, config_rev: 1, route, peers: [] };
  return { id: b.id, brief: b, state, members: null, config: { route: { exits: [] } }, configRev: 11, configMeta: null, bootId: null };
}

const routeExit = (idFull) => ({ id: idFull, id_full: idFull, name: idFull, online: true, exit: true, country: null, active: true });

// A slot that is not shown: no key and no state yet.
function hide(store, slot) {
  store.nets.find((n) => n.id === slot).id_hex = null;
  store.slots[slot].state = null;
}

function memoryStorage() {
  const data = new Map();
  return { getItem: (k) => data.get(k) ?? null, setItem: (k, v) => data.set(k, String(v)) };
}
