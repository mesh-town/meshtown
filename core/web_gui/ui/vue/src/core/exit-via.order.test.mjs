/* `saveExitIn` and `rememberCurrentExit` add to the remembered list instead of replacing it. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { getRememberedFor, setRememberedFor } from "../common/exits.ts";
import { rememberCurrentExit, saveExitIn } from "./exit-via.ts";
import { createStore } from "./store.ts";

const data = new Map();
globalThis.localStorage = { getItem: (k) => (data.has(k) ? data.get(k) : null), setItem: (k, v) => data.set(k, v) };

function setup() {
  const store = createStore();
  store.config = { route: { exits: [] } };
  store.configRev = 1;
  const api = async (method, path, body) => ({ config: body.config, rev: 2 });
  return { store, deps: { api, busTarget: new EventTarget() } };
}

describe("saveExitIn with the default remember", () => {
  it("B chosen with A remembered: [A, B]; A chosen again: still [A, B]", async () => {
    data.clear();
    setRememberedFor("main", ["A"]);
    const w = setup();
    await saveExitIn(w.store, "main", "B", w.deps);
    assert.deepEqual(getRememberedFor("main"), ["A", "B"]);
    await saveExitIn(w.store, "main", "A", w.deps);
    assert.deepEqual(getRememberedFor("main"), ["A", "B"]);
  });
});

describe("rememberCurrentExit", () => {
  it("the exit in play is added without dropping the others", () => {
    data.clear();
    setRememberedFor("main", ["A"]);
    const store = createStore();
    store.config = { route: { exits: ["B"] } };
    store.configRev = 1;
    rememberCurrentExit(store, (slot, ids) => assert.deepEqual([slot, ids], ["main", ["B"]]));
  });
});
