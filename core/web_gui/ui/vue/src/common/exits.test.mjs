/* `exits.ts`: the chosen exits and the remembered choice (the key is shared with the classic page). */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import {
  REMEMBERED_KEY,
  chosenExitIds,
  getRememberedEntries,
  getRememberedExits,
  getRememberedFor,
  pickChosenExits,
  setRememberedEntries,
  setRememberedExits,
  setRememberedFor,
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
    it("main entries are written as objects too", () => {
      const st = memoryStorage();
      setRememberedEntries([{ id: "abc", slot: "main" }], st);
      assert.equal(st.data[REMEMBERED_KEY], '[{"id":"abc","slot":"main"}]');
    });

    it("another network is an object with id and slot, like main", () => {
      const st = memoryStorage();
      setRememberedEntries([{ id: "abc", slot: "main" }, { id: "def", slot: NET }], st);
      assert.equal(st.data[REMEMBERED_KEY], `[{"id":"abc","slot":"main"},{"id":"def","slot":"${NET}"}]`);
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
      assert.equal(st.data[REMEMBERED_KEY], `[{"id":"x","slot":"main"},{"id":"y","slot":"${NET}"}]`);
    });

    it("setRememberedExits with no other networks writes objects", () => {
      const st = storageWith('["old"]');
      setRememberedExits(["x", "z"], st);
      assert.equal(st.data[REMEMBERED_KEY], '[{"id":"x","slot":"main"},{"id":"z","slot":"main"}]');
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

describe("remembered exits of one network (getRememberedFor / setRememberedFor)", () => {
  const NET = "k7q2xa";
  const OTHER = "m3n4pq";

  describe("getRememberedFor", () => {
    it("returns only the ids of the asked slot", () => {
      const st = storageWith(`["a",{"id":"b","slot":"${NET}"},{"id":"c","slot":"${OTHER}"},{"id":"d","slot":"${NET}"}]`);
      assert.deepEqual(getRememberedFor(NET, st), ["b", "d"]);
      assert.deepEqual(getRememberedFor(OTHER, st), ["c"]);
      assert.deepEqual(getRememberedFor("main", st), ["a"]);
    });

    it("a slot with no records — an empty list", () => {
      assert.deepEqual(getRememberedFor(NET, storageWith('["a"]')), []);
      assert.deepEqual(getRememberedFor(NET, memoryStorage()), []);
    });

    it("main equals getRememberedExits", () => {
      const st = storageWith(`["a",{"id":"b","slot":"${NET}"}]`);
      assert.deepEqual(getRememberedFor("main", st), getRememberedExits(st));
    });
  });

  describe("setRememberedFor", () => {
    it("writing a network keeps the main entries and the entries of other networks", () => {
      const st = storageWith(`["a",{"id":"o","slot":"${OTHER}"},{"id":"old","slot":"${NET}"}]`);
      setRememberedFor(NET, ["new"], st);
      assert.deepEqual(getRememberedFor("main", st), ["a"]);
      assert.deepEqual(getRememberedFor(OTHER, st), ["o"]);
      assert.deepEqual(getRememberedFor(NET, st), ["new"], "the old record of this slot is replaced");
    });

    it("written ids read back for the slot", () => {
      const st = memoryStorage();
      setRememberedFor(NET, ["x", "y"], st);
      assert.deepEqual(getRememberedFor(NET, st), ["x", "y"]);
    });

    it("an empty list clears the slot and leaves the others", () => {
      const st = storageWith(`["a",{"id":"b","slot":"${NET}"}]`);
      setRememberedFor(NET, [], st);
      assert.deepEqual(getRememberedFor(NET, st), []);
      assert.deepEqual(getRememberedFor("main", st), ["a"]);
    });

    it("main is stored as objects; a legacy bare id of main is replaced", () => {
      const st = storageWith('["old"]');
      setRememberedFor("main", ["x", "z"], st);
      assert.equal(st.data[REMEMBERED_KEY], '[{"id":"x","slot":"main"},{"id":"z","slot":"main"}]');
    });

    it("main with a network record: same bytes as setRememberedExits", () => {
      const raw = `["old",{"id":"y","slot":"${NET}"}]`;
      const viaFor = storageWith(raw);
      const viaExits = storageWith(raw);
      setRememberedFor("main", ["x"], viaFor);
      setRememberedExits(["x"], viaExits);
      assert.equal(viaFor.data[REMEMBERED_KEY], viaExits.data[REMEMBERED_KEY]);
    });

    it("an added network is stored as an object with id and slot", () => {
      const st = storageWith('["a"]');
      setRememberedFor(NET, ["b"], st);
      const stored = JSON.parse(st.data[REMEMBERED_KEY]);
      assert.ok(stored.some((e) => e.id === "a" && e.slot === "main"), "the legacy main entry becomes an object");
      assert.ok(stored.some((e) => e && e.id === "b" && e.slot === NET), "the network entry is {id, slot}");
    });
  });

  describe("storage throws", () => {
    const broken = { getItem() { throw new Error("x"); }, setItem() { throw new Error("x"); } };

    it("reading gives an empty list, writing is silent", () => {
      assert.deepEqual(getRememberedFor(NET, broken), []);
      assert.doesNotThrow(() => setRememberedFor(NET, ["a"], broken));
      assert.doesNotThrow(() => setRememberedFor("main", ["a"], broken));
    });
  });
});

function storageWith(raw) {
  return memoryStorage({ [REMEMBERED_KEY]: raw });
}
