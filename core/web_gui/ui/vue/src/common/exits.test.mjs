/* `exits.ts`: the chosen exits and the remembered choice (the key is shared with the classic page). */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import {
  REMEMBERED_KEY,
  chosenExitIds,
  getRememberedEntries,
  getRememberedExits,
  pickChosenExits,
  setRememberedEntries,
  setRememberedExits,
} from "./exits.ts";

function memoryStorage(initial = {}) {
  const data = { ...initial };
  return { data, getItem: (k) => (k in data ? data[k] : null), setItem: (k, v) => { data[k] = String(v); } };
}

describe("remembered exits", () => {
  it("same key as the classic page", () => assert.equal(REMEMBERED_KEY, "meshtown.exitsRemembered"));

  it("written and read back", () => {
    const st = memoryStorage();
    setRememberedExits(["a"], st);
    assert.deepEqual(getRememberedExits(st), ["a"]);
  });

  it("no entry, not an array, broken JSON — empty", () => {
    assert.deepEqual(getRememberedExits(memoryStorage()), []);
    assert.deepEqual(getRememberedExits(memoryStorage({ [REMEMBERED_KEY]: "{}" })), []);
    assert.deepEqual(getRememberedExits(memoryStorage({ [REMEMBERED_KEY]: "{" })), []);
  });

  it("storage throws — no exception", () => {
    const broken = { getItem() { throw new Error("x"); }, setItem() { throw new Error("x"); } };
    assert.deepEqual(getRememberedExits(broken), []);
    assert.doesNotThrow(() => setRememberedExits(["a"], broken));
  });
});

describe("pickChosenExits", () => {
  const route = { exits: [{ id_full: "R" }] };

  it("no snapshot or route — the config list", () => {
    assert.deepEqual(pickChosenExits(null, ["c"], 1), ["c"]);
    assert.deepEqual(pickChosenExits({}, ["c"], 1), ["c"]);
  });

  it("no config yet (rev null) — the snapshot route", () => {
    assert.deepEqual(pickChosenExits({ route, config_rev: 1 }, [], null), ["R"]);
  });

  it("snapshot not newer than the config — config; newer — snapshot", () => {
    assert.deepEqual(pickChosenExits({ route, config_rev: 3 }, ["c"], 3), ["c"]);
    assert.deepEqual(pickChosenExits({ route, config_rev: 4 }, ["c"], 3), ["R"]);
  });
});

describe("chosenExitIds", () => {
  it("takes the config and rev from the store", () => {
    const store = { state: { route: { exits: [] }, config_rev: 1 }, config: { route: { exits: ["c"] } }, configRev: 2 };
    assert.deepEqual(chosenExitIds(store), ["c"]);
  });
});

describe("remembered entries (per network)", () => {
  const NET = "k7q2xa";

  describe("reading", () => {
    it("a plain string is a main entry", () => {
      const st = storageWith('["abc"]');
      assert.deepEqual(getRememberedEntries(st), [{ id: "abc", slot: "main" }]);
    });

    it("an object with slot main is read as main", () => {
      const st = storageWith('[{"id":"abc","slot":"main"}]');
      assert.deepEqual(getRememberedEntries(st), [{ id: "abc", slot: "main" }]);
      assert.deepEqual(getRememberedExits(st), ["abc"]);
    });

    it("an object of another network keeps its slot, order is preserved", () => {
      const st = storageWith(`["a",{"id":"b","slot":"${NET}"},"c"]`);
      assert.deepEqual(getRememberedEntries(st), [
        { id: "a", slot: "main" },
        { id: "b", slot: NET },
        { id: "c", slot: "main" },
      ]);
    });

    it("garbage — empty list", () => {
      assert.deepEqual(getRememberedEntries(memoryStorage()), [], "no entry");
      assert.deepEqual(getRememberedEntries(storageWith("{")), [], "broken JSON");
      assert.deepEqual(getRememberedEntries(storageWith('{"id":"a"}')), [], "not an array");
      assert.deepEqual(getRememberedEntries(storageWith('"abc"')), [], "a bare string");
    });

    it("garbage items are skipped, valid ones stay", () => {
      const st = storageWith(`[1,null,{"id":5,"slot":"main"},{"id":"x","slot":"BAD!"},{"slot":"main"},"ok"]`);
      assert.deepEqual(getRememberedEntries(st), [{ id: "ok", slot: "main" }]);
    });

    it("getRememberedExits returns the main ids only", () => {
      const st = storageWith(`["a",{"id":"b","slot":"${NET}"},{"id":"c","slot":"main"}]`);
      assert.deepEqual(getRememberedExits(st), ["a", "c"]);
    });
  });

  describe("writing", () => {
    it("main entries are plain strings, byte for byte as before", () => {
      const st = memoryStorage();
      setRememberedEntries([{ id: "abc", slot: "main" }], st);
      assert.equal(st.data[REMEMBERED_KEY], '["abc"]');
    });

    it("another network is an object with id and slot, main untouched", () => {
      const st = memoryStorage();
      setRememberedEntries([{ id: "abc", slot: "main" }, { id: "def", slot: NET }], st);
      assert.equal(st.data[REMEMBERED_KEY], `["abc",{"id":"def","slot":"${NET}"}]`);
    });

    it("written entries read back unchanged", () => {
      const st = memoryStorage();
      const entries = [{ id: "d", slot: NET }, { id: "a", slot: "main" }];
      setRememberedEntries(entries, st);
      assert.deepEqual(getRememberedEntries(st), entries);
    });

    it("setRememberedExits keeps the entries of other networks after the new main ones", () => {
      const st = storageWith(`["old",{"id":"y","slot":"${NET}"}]`);
      setRememberedExits(["x"], st);
      assert.equal(st.data[REMEMBERED_KEY], `["x",{"id":"y","slot":"${NET}"}]`);
    });

    it("setRememberedExits with no other networks stays a plain string list", () => {
      const st = storageWith('["old"]');
      setRememberedExits(["x", "z"], st);
      assert.equal(st.data[REMEMBERED_KEY], '["x","z"]');
    });
  });

  describe("storage throws", () => {
    const broken = { getItem() { throw new Error("x"); }, setItem() { throw new Error("x"); } };

    it("reading gives an empty list, writing is silent", () => {
      assert.deepEqual(getRememberedEntries(broken), []);
      assert.doesNotThrow(() => setRememberedEntries([{ id: "a", slot: NET }], broken));
      assert.doesNotThrow(() => setRememberedExits(["a"], broken));
    });
  });
});

function storageWith(raw) {
  return memoryStorage({ [REMEMBERED_KEY]: raw });
}
