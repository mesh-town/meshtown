/* Pure logic behind the address column of "Ports shared with you": the ports a browser refuses and members with the same name. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { isBrowserBlockedPort, sameNameNodes } from "./ports-view.ts";

// The Fetch Standard list of ports a browser does not connect to.
const FETCH_BAD_PORTS = [
  1, 7, 9, 11, 13, 15, 17, 19, 20, 21, 22, 23, 25, 37, 42, 43, 53, 69, 77, 79, 87, 95, 101, 102, 103, 104, 109, 110, 111, 113, 115, 117, 119, 123,
  135, 137, 139, 143, 161, 179, 389, 427, 465, 512, 513, 514, 515, 526, 530, 531, 532, 540, 548, 554, 556, 563, 587, 601, 636, 989, 990, 993, 995,
  1719, 1720, 1723, 2049, 3659, 4045, 4190, 5060, 5061, 6000, 6566, 6665, 6666, 6667, 6668, 6669, 6679, 6697, 10080,
];

describe("isBrowserBlockedPort", () => {
  describe("a port of the Fetch Standard list", () => {
    it("is blocked, every one of them", () => {
      const missed = FETCH_BAD_PORTS.filter((port) => !isBrowserBlockedPort(port));
      assert.deepEqual(missed, []);
    });

    it("is blocked at the edges of the list: 1 and 10080", () => {
      assert.equal(isBrowserBlockedPort(1), true);
      assert.equal(isBrowserBlockedPort(10080), true);
    });
  });

  describe("a port outside the list", () => {
    it("is open for the browser: the usual web ports", () => {
      for (const port of [80, 443, 3000, 8000, 8080, 8443, 9000]) {
        assert.equal(isBrowserBlockedPort(port), false, "port " + port);
      }
    });

    it("is open for the browser when it is next to a blocked one", () => {
      for (const port of [2, 6, 24, 5062, 5999, 6001, 6664, 6670, 10079, 10081, 65535]) {
        assert.equal(isBrowserBlockedPort(port), false, "port " + port);
      }
    });
  });
});

describe("sameNameNodes", () => {
  const row = (node, node_name) => ({ node, node_name });

  it("is empty when every member has its own name", () => {
    assert.deepEqual([...sameNameNodes([row("aa", "finch"), row("bb", "alice")])], []);
  });

  it("holds both members when two different nodes show the same name", () => {
    const same = sameNameNodes([row("aa", "finch"), row("bb", "finch"), row("cc", "alice")]);
    assert.deepEqual([...same].sort(), ["aa", "bb"]);
  });

  it("is empty for several ports of one node: it is one member", () => {
    assert.deepEqual([...sameNameNodes([row("aa", "finch"), row("aa", "finch")])], []);
  });

  it("holds every node of a name shared by three members", () => {
    const same = sameNameNodes([row("aa", "x"), row("bb", "x"), row("cc", "x")]);
    assert.deepEqual([...same].sort(), ["aa", "bb", "cc"]);
  });

  it("compares the names that are shown: a blank name falls back to the id label, which differs per node", () => {
    assert.deepEqual([...sameNameNodes([row("aa", null), row("bb", "  ")])], []);
  });

  it("is empty for no rows", () => {
    assert.deepEqual([...sameNameNodes([])], []);
  });

  it("tells the name with a diacritic from the plain one", () => {
    assert.deepEqual([...sameNameNodes([row("aa", "Café Router"), row("bb", "Cafe Router")])], []);
  });
});
