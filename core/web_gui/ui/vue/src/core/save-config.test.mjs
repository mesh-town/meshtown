/* `save-config.ts`: a pinpoint config edit on top of the store, one retry on `stale_rev`,
 * a signal to the bridge only on success. A port of the cases of the classic `15-save-field.js`. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { reactive, ref } from "vue";
import { saveConfigFields, putConfigOnce, fieldErrorText } from "./save-config.ts";
import { NET_NAME } from "../common/words.ts";
import { ApiError, API_ERROR_TEXT } from "./api.ts";
import { EV_CHANGED } from "./bridge.ts";
import { createStore } from "./store.ts";

describe("saveConfigFields", () => {
  describe("success", () => {
    it("PUT 200 — {ok:true} and one meshtown:changed {what:'config'}", async () => {
      const w = setup({ script: [put200({ x: 1 }, 6)] });
      const res = await saveConfigFields(w.store, { x: 1 }, w.deps);
      assert.deepEqual(res, { ok: true });
      assert.deepEqual(w.events, [{ what: "config" }]);
    });

    it("after 200 the store has config and configRev from the response, configMeta — four lists", async () => {
      const w = setup({ script: [put200({ x: 1 }, 6, { read_only: ["a"], file: "c.toml" })] });
      await saveConfigFields(w.store, { x: 1 }, w.deps);
      assert.equal(w.store.configRev, 6);
      assert.deepEqual(plain(w.store.config), { x: 1 });
      assert.deepEqual(plain(w.store.configMeta), {
        read_only: ["a"],
        restart_fields: [],
        restart_pending: [],
        cli_overrides: [],
      });
      assert.equal(w.store.configFile, "c.toml");
    });

    it("configFile without file in the response — null", async () => {
      const w = setup({ script: [put200({}, 6)] });
      await saveConfigFields(w.store, { x: 1 }, w.deps);
      assert.equal(w.store.configFile, null);
    });
  });

  describe("pinpoint path", () => {
    it("{'a.b':1} — PUT has config.a.b = 1 and the store rev", async () => {
      const w = setup({ config: { a: { b: 0, c: 2 } }, rev: 5, script: [put200({ a: { b: 1, c: 2 } }, 6)] });
      await saveConfigFields(w.store, { "a.b": 1 }, w.deps);
      assert.deepEqual(w.calls[0].body, { rev: 5, config: { a: { b: 1, c: 2 } } });
    });

    it("store.config.a is untouched before the response, taken from the response after it", async () => {
      const gate = deferred();
      const w = setup({ config: { a: { b: 0 } }, rev: 5, script: [() => gate.promise] });
      const pending = saveConfigFields(w.store, { "a.b": 1 }, w.deps);
      await tick();
      assert.deepEqual(plain(w.store.config), { a: { b: 0 } }, "the store was changed before the server response");
      gate.resolve({ config: { a: { b: 1 } }, rev: 6 });
      await pending;
      assert.deepEqual(plain(w.store.config), { a: { b: 1 } });
      assert.equal(w.store.configRev, 6);
    });
  });

  describe("stale_rev", () => {
    it("first PUT stale_rev, GET returned a new rev — the second PUT goes with the new rev", async () => {
      const w = setup({
        rev: 5,
        script: [fail("stale_rev", 409), () => ({ config: { x: 0 }, rev: 9 }), put200({ x: 1 }, 10)],
      });
      const res = await saveConfigFields(w.store, { x: 1 }, w.deps);
      assert.deepEqual(w.calls.map((c) => `${c.method} ${c.path}`), ["PUT /api/config", "GET /api/config", "PUT /api/config"]);
      assert.equal(w.calls[2].body.rev, 9);
      assert.deepEqual(res, { ok: true });
    });

    it("stale_rev twice — {ok:false, error}, no third PUT, no events", async () => {
      const w = setup({
        script: [fail("stale_rev", 409), () => ({ config: {}, rev: 9 }), fail("stale_rev", 409)],
      });
      const res = await saveConfigFields(w.store, { x: 1 }, w.deps);
      assert.equal(res.ok, false);
      assert.equal(res.error.code, "stale_rev");
      assert.equal(w.calls.length, 3, "no requests must follow the second refusal");
      assert.deepEqual(w.events, []);
    });

    it("GET after stale_rev refused — no second PUT, {ok:false}", async () => {
      const w = setup({ script: [fail("stale_rev", 409), fail("network", 0)] });
      const res = await saveConfigFields(w.store, { x: 1 }, w.deps);
      assert.equal(res.ok, false);
      assert.deepEqual(w.calls.map((c) => c.method), ["PUT", "GET"]);
    });
  });

  describe("other PUT refusal", () => {
    it("{ok:false, error} with no retry and no events", async () => {
      const w = setup({ script: [fail("invalid", 422)] });
      const res = await saveConfigFields(w.store, { x: 1 }, w.deps);
      assert.equal(res.ok, false);
      assert.equal(res.error.code, "invalid");
      assert.equal(w.calls.length, 1);
      assert.deepEqual(w.events, []);
    });
  });

  describe("no store.config", () => {
    it("{ok:false}, no requests", async () => {
      const w = setup({ config: null, script: [] });
      const res = await saveConfigFields(w.store, { x: 1 }, w.deps);
      assert.equal(res.ok, false);
      assert.equal(res.error.code, "no_config");
      assert.equal(res.error.message, "Settings haven't been loaded yet");
      assert.equal(w.calls.length, 0);
    });
  });
});

describe("saveConfigFields: reactive input", () => {
  it("reactive and ref with a nested object — a real structuredClone with no exception", async () => {
    const w = setup({ script: [put200({ x: 1 }, 6)] });
    const api = async (method, path, body) => {
      const copy = structuredClone(body);
      w.calls.push(copy);
      return { config: copy.config, rev: 6 };
    };
    const changes = {
      "route.exits": ref(["a", { id: "b" }]).value,
      "ports.opened": reactive({ list: [{ port: 80 }] }),
    };
    const res = await saveConfigFields(w.store, changes, { api, busTarget: w.deps.busTarget });
    assert.deepEqual(res, { ok: true });
    assert.deepEqual(plain(w.calls[0].config.route.exits), ["a", { id: "b" }]);
    assert.deepEqual(plain(w.calls[0].config.ports.opened), { list: [{ port: 80 }] });
  });
});

describe("putConfigOnce", () => {
  describe("success", () => {
    it("one PUT on top of store.config, {ok:true}, one meshtown:changed {what:'config'}", async () => {
      const w = setup({ config: { a: { b: 0, c: 2 } }, rev: 5, script: [put200({ a: { b: 1, c: 2 } }, 6)] });
      const res = await putConfigOnce(w.store, { "a.b": 1 }, w.deps);
      assert.deepEqual(res, { ok: true });
      assert.deepEqual(w.calls.map((c) => `${c.method} ${c.path}`), ["PUT /api/config"]);
      assert.deepEqual(w.calls[0].body, { rev: 5, config: { a: { b: 1, c: 2 } } });
      assert.deepEqual(w.events, [{ what: "config" }]);
    });

    it("the response lands in the store: config, configRev, configMeta, configFile", async () => {
      const w = setup({ script: [put200({ x: 1 }, 6, { restart_pending: ["x"], file: "c.toml" })] });
      await putConfigOnce(w.store, { x: 1 }, w.deps);
      assert.equal(w.store.configRev, 6);
      assert.deepEqual(plain(w.store.config), { x: 1 });
      assert.deepEqual(plain(w.store.configMeta.restart_pending), ["x"]);
      assert.equal(w.store.configFile, "c.toml");
    });
  });

  describe("rev argument", () => {
    it("goes into the PUT body instead of store.configRev", async () => {
      const w = setup({ rev: 5, script: [put200({ x: 1 }, 10)] });
      await putConfigOnce(w.store, { x: 1 }, w.deps, 9);
      assert.equal(w.calls[0].body.rev, 9);
    });

    it("without the argument — store.configRev", async () => {
      const w = setup({ rev: 5, script: [put200({ x: 1 }, 6)] });
      await putConfigOnce(w.store, { x: 1 }, w.deps);
      assert.equal(w.calls[0].body.rev, 5);
    });
  });

  describe("stale_rev", () => {
    it("{ok:false, error}, one PUT, no GET, no events, the store intact", async () => {
      const w = setup({ config: { x: 0 }, rev: 5, script: [fail("stale_rev", 409)] });
      const res = await putConfigOnce(w.store, { x: 1 }, w.deps);
      assert.equal(res.ok, false);
      assert.equal(res.error.code, "stale_rev");
      assert.deepEqual(w.calls.map((c) => c.method), ["PUT"], "no retry and no GET expected");
      assert.deepEqual(w.events, []);
      assert.equal(w.store.configRev, 5);
      assert.deepEqual(plain(w.store.config), { x: 0 });
    });
  });

  describe("other refusal", () => {
    it("422 — {ok:false, error} with the body, no events", async () => {
      const w = setup({ script: [fail("invalid", 422)] });
      const res = await putConfigOnce(w.store, { x: 1 }, w.deps);
      assert.equal(res.ok, false);
      assert.equal(res.error.code, "invalid");
      assert.deepEqual(w.events, []);
    });
  });

  describe("no store.config", () => {
    it("{ok:false, no_config}, no requests", async () => {
      const w = setup({ config: null, script: [] });
      const res = await putConfigOnce(w.store, { x: 1 }, w.deps);
      assert.equal(res.ok, false);
      assert.equal(res.error.code, "no_config");
      assert.equal(w.calls.length, 0);
    });
  });

  describe("reactive input", () => {
    it("reactive with a nested array — a real structuredClone with no exception", async () => {
      const w = setup({ script: [put200({ x: 1 }, 6)] });
      const changes = { "z.arr": ref(["a", "b"]).value, "z.list": reactive({ items: [{ n: 1 }] }) };
      const res = await putConfigOnce(w.store, changes, w.deps);
      assert.deepEqual(res, { ok: true });
      assert.deepEqual(plain(w.calls[0].body.config.z.arr), ["a", "b"]);
    });
  });
});

describe("fieldErrorText", () => {
  it("422 invalid — the field text from body.fields by the first edit path", () => {
    const err = apiErr("invalid", 422, { fields: [{ path: "node.name", message: "too long" }] });
    assert.equal(fieldErrorText(err, { "node.name": "x" }), "too long");
  });

  it("other refusal — \"Couldn't save: <message>\"", () => {
    const err = apiErr("boom", 500, { message: "disk full" });
    assert.equal(fieldErrorText(err, { "node.name": "x" }), "Couldn't save: disk full");
  });

  it("no reason — \"Couldn't save — try again\"", () => {
    assert.equal(fieldErrorText(new ApiError(0, "x", ""), { "node.name": "x" }), "Couldn't save — try again");
  });

  it("422 without its own field in fields — also \"Couldn't save: …\"", () => {
    const err = apiErr("invalid", 422, { message: "bad", fields: [{ path: "other", message: "foreign" }] });
    assert.equal(fieldErrorText(err, { "node.name": "x" }), "Couldn't save: bad");
  });

  it("network and timeout — in plain English, no machine code", () => {
    const ch = { "node.name": "x" };
    assert.equal(fieldErrorText(new ApiError(0, "timeout", API_ERROR_TEXT.timeout), ch), "Couldn't save: " + NET_NAME + " didn't respond in time");
    assert.equal(fieldErrorText(new ApiError(0, "network", API_ERROR_TEXT.network), ch), "Couldn't save: " + NET_NAME + " is not responding");
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

/** `script` — one response per `api` call: a function (may throw or return a promise). */
function setup({ config = { x: 0 }, rev = 5, script }) {
  const store = createStore();
  store.config = config;
  store.configRev = rev;
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
