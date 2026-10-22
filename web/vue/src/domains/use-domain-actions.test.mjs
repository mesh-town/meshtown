/* `use-domain-actions.ts`: the wrapper accepts a stub `api` and returns reactive state. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { createStore } from "../core/store.ts";
import { useDomainActions } from "./use-domain-actions.ts";

describe("useDomainActions", () => {
  it("api from deps: add sends POST through it, the state is in ui", async () => {
    const calls = [];
    const a = useDomainActions(createStore(), { api: async (m, p, b) => void calls.push([m, p, b]) });
    assert.equal(await a.add("a.org"), true);
    assert.deepEqual(calls, [["POST", "/api/domains", { domain: "a.org" }]]);
    assert.equal(a.ui.checkAllBusy, false);
  });
});
