/* `autoExitPick`: which candidate the phone app picks on its own. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { autoExitPick } from "./exit-list.ts";

const row = (idFull, over = {}) => ({ idFull, label: idFull, country: null, online: true, active: false, path: null, rttMs: null, checked: false, ...over });

describe("autoExitPick", () => {
  it("no candidates — nothing", () => assert.equal(autoExitPick([]), null));

  it("the online one with the lowest latency wins", () => {
    assert.equal(autoExitPick([row("a", { rttMs: 80 }), row("b", { rttMs: 20 }), row("c", { rttMs: 50 })]), "b");
  });

  it("a row without latency goes last", () => {
    assert.equal(autoExitPick([row("a"), row("b", { rttMs: 300 })]), "b");
    assert.equal(autoExitPick([row("a"), row("b")]), "a");
  });

  it("offline and blocked rows are skipped", () => {
    const list = [row("a", { online: false, rttMs: 1 }), row("b", { blocked: true, rttMs: 2 }), row("c", { rttMs: 900 })];
    assert.equal(autoExitPick(list), "c");
    assert.equal(autoExitPick(list.slice(0, 2)), null);
  });
});
