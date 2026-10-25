/* `exitCandidates`: the row order does not depend on the choice. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { exitCandidates } from "./exit-list.ts";

const ids = (list) => list.map((item) => item.idFull);
const peer = (id) => ({ id_full: id, id: id + id, name: id, exit: true });
const state = { peers: [peer("a"), peer("b"), peer("c")] };
const remembered = ["r"];

describe("exitCandidates order", () => {
  const base = ["a", "b", "c", "r"];

  it("no choice: members in the peers order, then the remembered one", () => {
    assert.deepEqual(ids(exitCandidates(state, [], remembered)), base);
  });

  it("an offering member chosen: the rows stay in place", () => {
    const list = exitCandidates(state, ["b"], remembered);
    assert.deepEqual(ids(list), base);
    assert.equal(list[1].checked, true);
  });

  it("a remembered one chosen: the rows stay in place", () => {
    const list = exitCandidates(state, ["r"], remembered);
    assert.deepEqual(ids(list), base);
    assert.equal(list[3].checked, true);
  });

  it("a chosen id listed nowhere goes last", () => {
    assert.deepEqual(ids(exitCandidates(state, ["z"], remembered)), [...base, "z"]);
  });
});
