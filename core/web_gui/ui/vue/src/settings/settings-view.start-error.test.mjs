/* After a failed start the restart button is offered without any edit. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { restartTargets, saveBarView } from "./settings-view.ts";

const store = (over) => ({ restartSlots: [], configMeta: null, source: "node", state: { node: { start_error: null } }, ...over });
const failed = (over) => store({ state: { node: { start_error: "port 1091 for the browser proxy is in use" } }, ...over });

describe("restartTargets > failed start", () => {
  it("start_error set, nothing pending — the main slot", () => {
    assert.deepEqual(restartTargets(failed()), ["main"]);
  });

  it("start_error set, source is the overview — still the main slot", () => {
    assert.deepEqual(restartTargets(failed({ source: "overview" })), ["main"]);
  });

  it("start_error empty string or null — no slots", () => {
    assert.deepEqual(restartTargets(store({ state: { node: { start_error: "" } } })), []);
    assert.deepEqual(restartTargets(store()), []);
  });

  it("no state yet — no slots, no crash", () => {
    assert.deepEqual(restartTargets(store({ state: null })), []);
  });
});

describe("saveBarView > failed start", () => {
  const view = (requested) => saveBarView({ dirty: false, meta: null, configFile: null, requested, busy: false, message: { text: "", cls: "hint" }, restartTargets: restartTargets(failed()) });

  it("bar visible and restart offered with no edits", () => {
    const v = view(false);
    assert.equal(v.visible, true);
    assert.equal(v.restartOffered, true);
  });

  it("after the request — not offered", () => {
    assert.equal(view(true).restartOffered, false);
  });
});
