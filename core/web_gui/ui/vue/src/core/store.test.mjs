/* `store.ts`: a store of six snapshot fields, the "not saved" marks and the reasons for the node's silence (`absence`). */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { createStore, STORE_KEY } from "./store.ts";

describe("createStore", () => {
  it("six snapshot fields, settingsDirty and absence with initial values", () => {
    const store = createStore();
    assert.deepEqual(plain(store), {
      state: null,
      config: null,
      configRev: null,
      configMeta: null,
      configFile: null,
      activeSection: "home",
      settingsDirty: false,
      absence: null,
      members: null,
      slots: {},
      nets: [],
      restartSlots: [],
      notices: [],
      lead: "main",
      peersTab: null,
      netSub: "members",
      source: null,
    });
  });

  it("absence — null: while the page receives snapshots there is no reason for silence", () => {
    assert.equal(createStore().absence, null);
  });

  it("absence — a store field, not a stray property: the key exists at once, before the first write", () => {
    assert.ok("absence" in createStore(), "createStore() must declare absence so that a write from useShell is reactive");
  });

  it("STORE_KEY — a symbol for provide/inject", () => {
    assert.equal(typeof STORE_KEY, "symbol");
  });
});

function plain(value) {
  return JSON.parse(JSON.stringify(value));
}
