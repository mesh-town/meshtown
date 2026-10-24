/* A remembered exit is kept in the groups of several networks only while the network still offers it. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { exitCandidates, offeredRemembered } from "./exit-list.ts";

const peer = (idFull, exit, self) => ({ id_full: idFull, id: idFull.slice(0, 4), name: "n-" + idFull, exit, self });

describe("offeredRemembered", () => {
  it("an id that is not among the peers is dropped", () => {
    assert.deepEqual(offeredRemembered({ peers: [peer("p", true)] }, ["foreign"]), []);
  });

  it("an id of a peer that does not offer an exit is dropped", () => {
    assert.deepEqual(offeredRemembered({ peers: [peer("former", false)] }, ["former"]), []);
  });

  it("an id of a peer that offers an exit stays", () => {
    assert.deepEqual(offeredRemembered({ peers: [peer("p", true)] }, ["p", "x"]), ["p"]);
  });

  it("the own node does not count as offering", () => {
    assert.deepEqual(offeredRemembered({ peers: [peer("me", true, true)] }, ["me"]), []);
  });

  it("without a state or a peer list the ids stay", () => {
    assert.deepEqual(offeredRemembered(null, ["r"]), ["r"]);
    assert.deepEqual(offeredRemembered({}, ["r"]), ["r"]);
  });
});

describe("exitCandidates with one network", () => {
  it("lists a remembered id that has no peer", () => {
    const list = exitCandidates({ peers: [peer("p", true)] }, [], ["foreign"]);
    assert.deepEqual(list.map((c) => c.idFull), ["foreign", "p"]);
  });
});
