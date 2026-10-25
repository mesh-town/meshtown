/* `statePanel` with several networks follows the header verdict handed in `ctx.head`. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { statePanel } from "./home-view.ts";
import { baseState } from "../testing/section.mjs";

const ctx = (head) => ({ multi: true, netOf: () => "work", head });
const RED = { level: "red", text: "Sites won't open" };
const GREEN = { level: "green", text: "Sites open via x (work)" };

describe("statePanel with ctx.head", () => {
  it("a red head: a red panel with the head text and the pick-other action", () => {
    const p = statePanel(baseState(), null, null, ctx(RED));
    assert.equal(p.level, "red");
    assert.equal(p.main, RED.text);
    assert.deepEqual(p.actions, ["pick-other"]);
  });

  it("a green head: a green panel with the head text", () => {
    const p = statePanel(baseState(), null, null, ctx(GREEN));
    assert.equal(p.level, "green");
    assert.equal(p.main, GREEN.text);
  });

  it("a null head: the usual panel", () => {
    assert.equal(statePanel(baseState(), null, null, ctx(null)).level, "green");
  });

  it("a red head with one network: ignored", () => {
    const p = statePanel(baseState(), null, null, { ...ctx(RED), multi: false });
    assert.equal(p.level, "green");
  });
});
