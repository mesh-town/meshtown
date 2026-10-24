/* Reading the remembered exits per network: an old bare id belongs to `main`, an entry of another network never leaks into it. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { REMEMBERED_KEY, getRememberedFor } from "./exits.ts";

const storageOf = (value) => ({ getItem: (k) => (k === REMEMBERED_KEY ? JSON.stringify(value) : null), setItem: () => {} });

describe("remembered exits per network", () => {
  it("an old bare id is the memory of main only", () => {
    const st = storageOf(["old-id"]);
    assert.deepEqual(getRememberedFor("main", st), ["old-id"]);
    assert.deepEqual(getRememberedFor("k7q2xa", st), []);
  });

  it("an entry of another network is not in main", () => {
    const st = storageOf([{ id: "x", slot: "k7q2xa" }]);
    assert.deepEqual(getRememberedFor("main", st), []);
    assert.deepEqual(getRememberedFor("k7q2xa", st), ["x"]);
  });
});
