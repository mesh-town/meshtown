/* `save-config.ts`, writing into network slots: one edit in one slot, the same edit in every network
 * that holds a key, the failure of one network not stopping the rest. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { saveSlotFields, eachSlot, slotFailureText, targetSlots, asResult } from "./save-config.ts";
import { ApiError } from "./api.ts";
import { EV_CHANGED } from "./bridge.ts";
import { createStore } from "./store.ts";

const B = "k7q2xa";
const C = "m3n4pq";

describe("saveSlotFields", () => {
  describe("main slot", () => {
    it("PUT /api/config with the main revision, bus gets 'config'", async () => {
      const w = setup({ script: [put200({ x: 1 }, 6)] });
      const res = await saveSlotFields(w.store, "main", { x: 1 }, w.deps);
      assert.deepEqual(res, { ok: true });
      assert.deepEqual(w.calls.map((c) => `${c.method} ${c.path}`), ["PUT /api/config"]);
      assert.equal(w.calls[0].body.rev, 5);
      assert.deepEqual(w.events, [{ what: "config" }]);
    });
  });

  describe("another slot", () => {
    it("PUT /api/net/<slot>/config with the slice revision and the edited slice config", async () => {
      const w = setup({ sliceB: { config: { a: { b: 0, c: 2 } }, configRev: 11 }, script: [put200({ a: { b: 1, c: 2 } }, 12)] });
      const res = await saveSlotFields(w.store, B, { "a.b": 1 }, w.deps);
      assert.deepEqual(res, { ok: true });
      assert.deepEqual(w.calls.map((c) => `${c.method} ${c.path}`), [`PUT /api/net/${B}/config`]);
      assert.deepEqual(w.calls[0].body, { rev: 11, config: { a: { b: 1, c: 2 } } });
    });

    it("the reply lands in the slice: config, configRev, configMeta, boot of the slot state", async () => {
      const w = setup({ sliceB: { state: { boot_id: "boot-7" } }, script: [put200({ x: 1 }, 12, { read_only: ["r"] })] });
      await saveSlotFields(w.store, B, { x: 1 }, w.deps);
      const slice = w.store.slots[B];
      assert.deepEqual(plain(slice.config), { x: 1 });
      assert.equal(slice.configRev, 12);
      assert.deepEqual(plain(slice.configMeta.read_only), ["r"]);
      assert.equal(slice.bootId, "boot-7");
    });

    it("the main config and revision stay as they were", async () => {
      const w = setup({ script: [put200({ x: 1 }, 12)] });
      await saveSlotFields(w.store, B, { x: 1 }, w.deps);
      assert.deepEqual(plain(w.store.config), { x: 0 });
      assert.equal(w.store.configRev, 5);
    });

    it("the bus gets 'state' once and never 'config'", async () => {
      const w = setup({ script: [put200({ x: 1 }, 12)] });
      await saveSlotFields(w.store, B, { x: 1 }, w.deps);
      assert.deepEqual(w.events, [{ what: "state" }]);
    });

    it("the slice config is untouched until the response arrives", async () => {
      const gate = deferred();
      const w = setup({ sliceB: { config: { a: { b: 0 } } }, script: [() => gate.promise] });
      const pending = saveSlotFields(w.store, B, { "a.b": 1 }, w.deps);
      await tick();
      assert.deepEqual(plain(w.store.slots[B].config), { a: { b: 0 } }, "the slice changed before the server answered");
      gate.resolve({ config: { a: { b: 1 } }, rev: 12 });
      await pending;
      assert.deepEqual(plain(w.store.slots[B].config), { a: { b: 1 } });
    });
  });

  describe("stale_rev", () => {
    it("GET of the slot config, then one more PUT with the new revision", async () => {
      const w = setup({
        script: [fail("stale_rev", 409), () => ({ config: { x: 0 }, rev: 20 }), put200({ x: 1 }, 21)],
      });
      const res = await saveSlotFields(w.store, B, { x: 1 }, w.deps);
      assert.deepEqual(w.calls.map((c) => `${c.method} ${c.path}`), [
        `PUT /api/net/${B}/config`,
        `GET /api/net/${B}/config`,
        `PUT /api/net/${B}/config`,
      ]);
      assert.equal(w.calls[2].body.rev, 20);
      assert.deepEqual(res, { ok: true });
      assert.equal(w.store.slots[B].configRev, 21);
    });

    it("the second stale_rev — {ok:false, error}, no third PUT, no events", async () => {
      const w = setup({
        script: [fail("stale_rev", 409), () => ({ config: {}, rev: 20 }), fail("stale_rev", 409)],
      });
      const res = await saveSlotFields(w.store, B, { x: 1 }, w.deps);
      assert.equal(res.ok, false);
      assert.equal(res.error.code, "stale_rev");
      assert.equal(w.calls.length, 3, "no requests must follow the second refusal");
      assert.deepEqual(w.events, []);
    });
  });

  describe("other refusal", () => {
    it("422 — {ok:false, error}, one request, no events, slice intact", async () => {
      const w = setup({ script: [fail("invalid", 422)] });
      const res = await saveSlotFields(w.store, B, { x: 1 }, w.deps);
      assert.equal(res.ok, false);
      assert.equal(res.error.code, "invalid");
      assert.equal(w.calls.length, 1);
      assert.deepEqual(w.events, []);
      assert.equal(w.store.slots[B].configRev, 11);
    });
  });

  describe("slice without a config", () => {
    it("no_config, no requests, no events", async () => {
      const w = setup({ sliceB: { config: null, configRev: null }, script: [] });
      const res = await saveSlotFields(w.store, B, { x: 1 }, w.deps);
      assert.equal(res.ok, false);
      assert.equal(res.error.code, "no_config");
      assert.equal(res.error.message, "Settings haven't been loaded yet");
      assert.equal(w.calls.length, 0);
      assert.deepEqual(w.events, []);
    });

    it("a slot with no slice at all — also no_config without requests", async () => {
      const w = setup({ script: [] });
      const res = await saveSlotFields(w.store, C, { x: 1 }, w.deps);
      assert.equal(res.ok, false);
      assert.equal(res.error.code, "no_config");
      assert.equal(w.calls.length, 0);
    });
  });
});

describe("eachSlot", () => {
  describe("order", () => {
    it("[B, main] — main is written first", async () => {
      const w = setup({ script: [] });
      const order = [];
      await eachSlot(w.store, [B, "main"], async (slot) => (order.push(slot), { ok: true }));
      assert.deepEqual(order, ["main", B]);
    });

    it("other slots keep the given order", async () => {
      const w = setup({ script: [] });
      const order = [];
      await eachSlot(w.store, [C, B], async (slot) => (order.push(slot), { ok: true }));
      assert.deepEqual(order, [C, B]);
    });

    it("one at a time: the next write starts after the previous one finished", async () => {
      const w = setup({ script: [] });
      const gate = deferred();
      const started = [];
      const pending = eachSlot(w.store, ["main", B], (slot) => {
        started.push(slot);
        return slot === "main" ? gate.promise : Promise.resolve({ ok: true });
      });
      await tick();
      assert.deepEqual(started, ["main"], "the second write started before the first one finished");
      gate.resolve({ ok: true });
      await pending;
      assert.deepEqual(started, ["main", B]);
    });
  });

  describe("every write succeeded", () => {
    it("no failures", async () => {
      const w = setup({ script: [] });
      const failures = await eachSlot(w.store, ["main", B], async () => ({ ok: true }));
      assert.deepEqual(failures, []);
    });
  });

  describe("the second write failed", () => {
    it("the first is written, the failure names the second network by its label", async () => {
      const w = setup({ script: [put200({ x: 1 }, 6), fail("invalid", 422)] });
      const failures = await eachSlot(w.store, ["main", B], (slot) => saveSlotFields(w.store, slot, { x: 1 }, w.deps));
      assert.equal(failures.length, 1);
      assert.equal(failures[0].slot, B);
      assert.equal(failures[0].label, "friends");
      assert.equal(failures[0].error.code, "invalid");
      assert.deepEqual(plain(w.store.config), { x: 1 }, "main must stay written");
      assert.equal(w.store.configRev, 6);
    });

    it("the failure of the first does not stop the second", async () => {
      const w = setup({ script: [] });
      const seen = [];
      const failures = await eachSlot(w.store, ["main", B], async (slot) => {
        seen.push(slot);
        return slot === "main" ? { ok: false, error: new ApiError(0, "boom", "boom") } : { ok: true };
      });
      assert.deepEqual(seen, ["main", B]);
      assert.deepEqual(failures.map((f) => f.slot), ["main"]);
    });
  });

  describe("write throws", () => {
    it("an exception is a failure of that slot, the rest still run", async () => {
      const w = setup({ script: [] });
      const seen = [];
      const failures = await eachSlot(w.store, ["main", B], async (slot) => {
        seen.push(slot);
        if (slot === "main") throw new ApiError(0, "network", "offline");
        return { ok: true };
      });
      assert.deepEqual(seen, ["main", B]);
      assert.equal(failures.length, 1);
      assert.equal(failures[0].slot, "main");
      assert.equal(failures[0].error.message, "offline");
    });

    it("a synchronous throw is a failure too", async () => {
      const w = setup({ script: [] });
      const failures = await eachSlot(w.store, [B], () => {
        throw new ApiError(0, "boom", "sync boom");
      });
      assert.deepEqual(failures.map((f) => f.error.message), ["sync boom"]);
    });
  });
});

describe("slotFailureText", () => {
  it("quotes the network label: \"friends\": boom", () => {
    const failure = { slot: B, label: "friends", error: new ApiError(0, "x", "boom") };
    assert.equal(slotFailureText(failure), "\"friends\": boom");
  });
});

describe("targetSlots", () => {
  it("main first even when the overview lists it later", () => {
    const w = setup({ script: [], mainKey: true, thirdKey: true });
    w.store.nets = [w.store.nets.find((n) => n.id === C), w.store.nets.find((n) => n.id === B), w.store.nets.find((n) => n.id === "main")];
    assert.deepEqual(targetSlots(w.store), ["main", C, B]);
  });

  it("a visible slot without a key is left out", () => {
    const w = setup({ script: [], mainKey: false });
    assert.deepEqual(targetSlots(w.store), [B]);
  });

  it("a network being joined is left out", () => {
    const w = setup({ script: [], mainKey: true, bJoining: true });
    assert.deepEqual(targetSlots(w.store), ["main"]);
  });

  it("a network whose state has not arrived is not visible, so it is left out", () => {
    const w = setup({ script: [], mainKey: true });
    w.store.slots[B].state = null;
    assert.deepEqual(targetSlots(w.store), ["main"]);
  });

  it("one network — only the lead", () => {
    const w = setup({ script: [], mainKey: false });
    w.store.lead = B;
    assert.deepEqual(targetSlots(w.store), [w.store.lead]);
  });
});

describe("a shared path written into every network", () => {
  it("main first, the second network fails: main is saved, the error carries the label", async () => {
    const w = setup({ script: [put200({ x: 1 }, 6), fail("invalid", 422)], mainKey: true });
    const failures = await eachSlot(w.store, targetSlots(w.store), (slot) => saveSlotFields(w.store, slot, { x: 1 }, w.deps));
    assert.deepEqual(w.calls.map((c) => c.path), ["/api/config", `/api/net/${B}/config`]);
    assert.deepEqual(failures.map(slotFailureText), [`"friends": invalid`]);
    assert.equal(w.store.configRev, 6);
  });
});

describe("asResult", () => {
  it("no value — success", async () => {
    assert.deepEqual(await asResult(Promise.resolve()), { ok: true });
  });

  it("a SaveResult passes through as is", async () => {
    const failed = { ok: false, error: new ApiError(0, "x", "no") };
    assert.equal(await asResult(Promise.resolve(failed)), failed);
    assert.deepEqual(await asResult(Promise.resolve({ ok: true })), { ok: true });
  });

  it("a rejection — {ok:false, error}", async () => {
    const error = new ApiError(0, "network", "offline");
    assert.deepEqual(await asResult(Promise.reject(error)), { ok: false, error });
  });
});

/** An ApiError-shaped error (`status, code, message, body`) — independent of the constructor argument order. */
function apiErr(code, status, body = null) {
  return Object.assign(new Error((body && body.message) || code), { status, code, body });
}

function fail(code, status) {
  return () => {
    throw apiErr(code, status, { error: code });
  };
}

function put200(config, rev, extra = {}) {
  return () => ({ config, rev, ...extra });
}

function brief(id, over = {}) {
  return {
    id, label: null, id_hex: null, status: "running", exit_enabled: false, proxy: id === "main",
    route_state: "direct", exits: [], members_voting: 1, joining: false, members_admitting: 0,
    me_excluded: false, me_pending: false, peers_online: 0, peers_total: 0, ...over,
  };
}

function slice(b, over = {}) {
  return { id: b.id, brief: b, state: {}, members: null, config: { x: 0 }, configRev: 11, configMeta: null, bootId: null, ...over };
}

/**
 * A store with main (revision 5, key when `mainKey`) and network B "friends" (with a key, slice revision 11);
 * `thirdKey` adds C with a key. `script` — one response per `api` call.
 */
function setup({ script, mainKey = false, thirdKey = false, bJoining = false, sliceB = {} }) {
  const store = createStore();
  store.config = { x: 0 };
  store.configRev = 5;
  const nets = [
    brief("main", { id_hex: mainKey ? "aa00bb" : null }),
    brief(B, { label: "friends", id_hex: "3f2a00", joining: bJoining }),
  ];
  if (thirdKey) nets.push(brief(C, { label: "work", id_hex: "9c1d00" }));
  store.nets = nets;
  for (const b of nets) store.slots[b.id] = slice(b, b.id === B ? sliceB : {});
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
  return { store, calls, events, deps: { api, busTarget: target } };
}

function deferred() {
  let resolve;
  const promise = new Promise((r) => (resolve = r));
  return { promise, resolve };
}

const tick = () => new Promise((r) => setImmediate(r));

function plain(value) {
  return JSON.parse(JSON.stringify(value));
}
