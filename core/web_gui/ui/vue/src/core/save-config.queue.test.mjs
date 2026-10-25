/* `save-config.ts`: writes to one store go out one at a time; each reads the revision and the config when it is sent. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { saveConfigFields, putConfigOnce, saveSlotFields } from "./save-config.ts";
import { ApiError } from "./api.ts";
import { createStore } from "./store.ts";

const SLOT = "bbbb";
const SLOT_PATH = `/api/net/${SLOT}/config`;

function gate() {
  let open;
  const promise = new Promise((r) => (open = r));
  return { promise, open };
}

/** A server stub: 409 `stale_rev` on any revision but the current one, +1 revision per write.
 * `failures[path]` — a list of errors for the next writes to that path; `gates[path]` — holds the first reply. */
function setup({ failures = {}, gates = {} } = {}) {
  const store = createStore();
  store.config = { a: 0, b: 0 };
  store.configRev = 5;
  store.slots[SLOT] = { id: SLOT, brief: null, state: {}, members: null, config: { s: 0, t: 0 }, configRev: 11, configMeta: null, bootId: null };
  const server = { "/api/config": { config: { a: 0, b: 0 }, rev: 5 }, [SLOT_PATH]: { config: { s: 0, t: 0 }, rev: 11 } };
  const calls = [];
  const held = { ...gates };
  const api = async (method, path, body) => {
    calls.push({ method, path, body: body === undefined ? undefined : structuredClone(body) });
    const cur = server[path];
    if (method === "GET") return structuredClone(cur);
    const fail = (failures[path] || []).shift();
    if (fail) throw fail;
    if (body.rev !== cur.rev) throw new ApiError(409, "stale_rev", "stale", { rev: cur.rev });
    server[path] = { config: structuredClone(body.config), rev: cur.rev + 1 };
    const reply = structuredClone(server[path]);
    const g = held[path];
    delete held[path];
    if (g) await g.promise;
    return reply;
  };
  return { store, calls, deps: { api, busTarget: new EventTarget() } };
}

const puts = (w, path) => w.calls.filter((c) => c.method === "PUT" && c.path === path);

describe("main config writes", () => {
  it("saveConfigFields then putConfigOnce: the second sees the revision and the config of the first", async () => {
    const g = gate();
    const w = setup({ gates: { "/api/config": g } });
    const first = saveConfigFields(w.store, { a: 1 }, w.deps);
    const second = putConfigOnce(w.store, { b: 2 }, w.deps);
    await new Promise((r) => setImmediate(r));
    assert.equal(puts(w, "/api/config").length, 1, "the second write waits for the first");
    g.open();
    assert.deepEqual(await first, { ok: true });
    assert.deepEqual(await second, { ok: true });
    const sent = puts(w, "/api/config");
    assert.equal(sent.length, 2);
    assert.equal(sent[1].body.rev, 6);
    assert.deepEqual(sent[1].body.config, { a: 1, b: 2 });
    assert.deepEqual(w.calls.map((c) => c.method), ["PUT", "PUT"], "no 409, no GET");
  });

  it("two putConfigOnce calls in a row: both succeed", async () => {
    const g = gate();
    const w = setup({ gates: { "/api/config": g } });
    const first = putConfigOnce(w.store, { a: 1 }, w.deps);
    const second = putConfigOnce(w.store, { b: 2 }, w.deps);
    g.open();
    assert.equal((await first).ok, true);
    assert.equal((await second).ok, true);
    assert.equal(w.store.configRev, 7);
  });

  it("an explicit revision is sent as given", async () => {
    const w = setup();
    const res = await putConfigOnce(w.store, { a: 1 }, w.deps, 9);
    assert.equal(res.ok, false);
    assert.equal(w.calls[0].body.rev, 9);
  });

  it("the first write failed (500): the second still goes out with the old revision", async () => {
    const g = gate();
    const w = setup({ failures: { "/api/config": [new ApiError(500, "boom", "boom")] }, gates: { "/api/config": g } });
    const first = putConfigOnce(w.store, { a: 1 }, w.deps);
    const second = putConfigOnce(w.store, { b: 2 }, w.deps);
    g.open();
    assert.equal((await first).ok, false);
    assert.equal((await second).ok, true);
    const sent = puts(w, "/api/config");
    assert.equal(sent.length, 2);
    assert.equal(sent[1].body.rev, 5);
    assert.deepEqual(sent[1].body.config, { a: 0, b: 2 });
  });

  it("the first write was refused (invalid): the second still goes out", async () => {
    const w = setup({ failures: { "/api/config": [new ApiError(422, "invalid", "bad", { fields: [] })] } });
    const first = saveConfigFields(w.store, { a: 1 }, w.deps);
    const second = saveConfigFields(w.store, { b: 2 }, w.deps);
    assert.equal((await first).ok, false);
    assert.equal((await second).ok, true);
    assert.equal(puts(w, "/api/config").length, 2);
  });
});

describe("slot writes", () => {
  it("two writes to one slot: the second carries the revision of the first reply", async () => {
    const g = gate();
    const w = setup({ gates: { [SLOT_PATH]: g } });
    const first = saveSlotFields(w.store, SLOT, { s: 1 }, w.deps);
    const second = saveSlotFields(w.store, SLOT, { t: 2 }, w.deps);
    g.open();
    assert.equal((await first).ok, true);
    assert.equal((await second).ok, true);
    const sent = puts(w, SLOT_PATH);
    assert.equal(sent.length, 2);
    assert.equal(sent[1].body.rev, 12);
    assert.deepEqual(sent[1].body.config, { s: 1, t: 2 });
    assert.deepEqual(w.calls.map((c) => c.method), ["PUT", "PUT"]);
  });

  it("a slot write and a main write: each goes out in order and succeeds", async () => {
    const g = gate();
    const w = setup({ gates: { "/api/config": g } });
    const main = saveConfigFields(w.store, { a: 1 }, w.deps);
    const slot = saveSlotFields(w.store, SLOT, { s: 1 }, w.deps);
    const main2 = saveSlotFields(w.store, "main", { b: 2 }, w.deps);
    g.open();
    assert.equal((await main).ok, true);
    assert.equal((await slot).ok, true);
    assert.equal((await main2).ok, true);
    assert.deepEqual(puts(w, "/api/config").map((c) => c.body.rev), [5, 6]);
    assert.equal(w.calls.filter((c) => c.method === "GET").length, 0);
  });
});
