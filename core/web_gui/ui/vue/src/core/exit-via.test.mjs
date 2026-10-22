/* `exit-via.ts`: the status and remembering after "Exit via …". */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "./api.ts";
import { saveSingleExit } from "./exit-via.ts";
import { createStore } from "./store.ts";

function setup(fail = null) {
  const store = createStore();
  store.config = { route: { exits: [] } };
  store.configRev = 1;
  const calls = [];
  const remembered = [];
  const api = async (method, path, body) => {
    calls.push(structuredClone(body));
    if (fail) throw fail;
    return { config: body.config, rev: 2 };
  };
  return { store, calls, remembered, deps: { api, busTarget: new EventTarget(), remember: (x) => remembered.push(x) } };
}

describe("saveSingleExit", () => {
  it("success: PUT with the single exit, \"Saved\", the choice is remembered", async () => {
    const w = setup();
    const st = await saveSingleExit(w.store, "AA", w.deps);
    assert.deepEqual(st, { text: "Saved", cls: "saved" });
    assert.deepEqual(w.calls[0].config.route.exits, ["AA"]);
    assert.deepEqual(w.remembered, [["AA"]]);
  });

  it("refusal: the field error text, nothing is remembered", async () => {
    const w = setup(new ApiError(500, "boom", "failure"));
    const st = await saveSingleExit(w.store, "AA", w.deps);
    assert.equal(st.cls, "field-error-text");
    assert.match(st.text, /^Couldn't save: /);
    assert.deepEqual(w.remembered, []);
  });
});
