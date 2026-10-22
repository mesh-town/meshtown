/* `exits.ts`: the chosen exits and the remembered choice (the key is shared with the classic page). */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { REMEMBERED_KEY, chosenExitIds, getRememberedExits, pickChosenExits, setRememberedExits } from "./exits.ts";

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
