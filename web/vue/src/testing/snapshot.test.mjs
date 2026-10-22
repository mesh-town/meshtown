/* `snapshot.js`: applying a snapshot to the store for tests. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { createStore } from "../core/store.ts";
import { applySnapshot } from "./snapshot.mjs";

describe("applySnapshot", () => {
  describe("full snapshot", () => {
    it("replaces fields entirely", () => {
      const store = createStore();
      applySnapshot(store, {
        state: { a: 1 },
        config: { b: 2 },
        configRev: 3,
        configMeta: { read_only: [] },
        configFile: "f.toml",
        activeSection: "log",
      });
      assert.deepEqual(plain(store), {
        state: { a: 1 },
        config: { b: 2 },
        configRev: 3,
        configMeta: { read_only: [] },
        configFile: "f.toml",
        activeSection: "log",
        settingsDirty: false,
        absence: null,
        members: null,
      });
    });

    it("settingsDirty is left alone by the snapshot: neither set nor reset", () => {
      const store = createStore();
      store.settingsDirty = true;
      applySnapshot(store, { state: { a: 1 }, settingsDirty: false });
      assert.equal(store.settingsDirty, true);
    });
  });

  describe("snapshot without some keys", () => {
    it("a missing config keeps the previous one", () => {
      const store = createStore();
      applySnapshot(store, { config: { keep: true } });
      applySnapshot(store, { state: { n: 1 } });
      assert.deepEqual(plain(store.config), { keep: true });
      assert.deepEqual(plain(store.state), { n: 1 });
    });
  });

  describe("unknown key", () => {
    it("does not get into the store", () => {
      const store = createStore();
      applySnapshot(store, { state: {}, unknown: 1 });
      assert.equal("unknown" in store, false);
    });
  });

  describe("detail is not an object", () => {
    it("null, string, number — the store does not change", () => {
      const store = createStore();
      for (const bad of [null, undefined, "x", 5]) applySnapshot(store, bad);
      assert.equal(store.state, null);
      assert.equal(store.activeSection, "home");
    });
  });
});

function plain(value) {
  return JSON.parse(JSON.stringify(value));
}
