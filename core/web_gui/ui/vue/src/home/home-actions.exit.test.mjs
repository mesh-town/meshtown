/* `chooseExit` and `chooseDirect` add to the remembered list: the rows keep their order. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { getRememberedFor, setRememberedFor } from "../common/exits.ts";
import { createStore } from "../core/store.ts";
import { chooseDirect, chooseExit, createHomeForm } from "./home-actions.ts";

Object.defineProperty(globalThis, "document", { value: new EventTarget(), configurable: true });

function setup(exits, remembered) {
  const data = new Map();
  const storage = { getItem: (k) => (data.has(k) ? data.get(k) : null), setItem: (k, v) => data.set(k, v) };
  setRememberedFor("main", remembered, storage);
  const store = createStore();
  store.state = { network: { label: "home", id_hex: "n" }, peers: [] };
  store.config = { route: { exits: [...exits] }, node: { name: "x" }, exit: { enabled: false } };
  store.configRev = 1;
  const api = async (method, path, body) => ({ config: structuredClone(body.config), rev: 2 });
  return { storage, store, form: createHomeForm(), deps: { api, storage, setTimer: () => 1, clearTimer: () => {} } };
}

describe("chooseExit remembering", () => {
  it("B chosen with A remembered: [A, B]; A chosen again: [A, B]", async () => {
    const w = setup([], ["A"]);
    await chooseExit(w.form, w.store, "B", w.deps);
    assert.deepEqual(getRememberedFor("main", w.storage), ["A", "B"]);
    await chooseExit(w.form, w.store, "A", w.deps);
    assert.deepEqual(getRememberedFor("main", w.storage), ["A", "B"]);
  });
});

describe("chooseDirect remembering", () => {
  it("\"Directly\" after A: A stays remembered once, the order is kept", async () => {
    const w = setup(["A"], ["B", "A"]);
    await chooseDirect(w.form, w.store, w.deps);
    assert.deepEqual(getRememberedFor("main", w.storage), ["B", "A"]);
  });
});
