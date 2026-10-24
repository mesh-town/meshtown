/* A stale copy of the config can name a network the node has already left (`unknown_net`, refused before the
 * revision is checked): the save re-reads the config and retries once. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "./api.ts";
import { saveConfigFields } from "./save-config.ts";
import { createStore } from "./store.ts";

function setup(script) {
  const store = createStore();
  store.config = { proxy: { net: "gone" }, a: 0 };
  store.configRev = 3;
  const calls = [];
  const events = [];
  const busTarget = new EventTarget();
  busTarget.addEventListener("meshtown:changed", (e) => events.push(e.detail));
  const api = async (method, path, body) => {
    calls.push({ method, path, body });
    return script.shift()(body);
  };
  return { store, calls, events, deps: { api, busTarget } };
}

const unknown = () => { throw new ApiError(404, "unknown_net", "unknown network"); };
const fresh = () => ({ config: { proxy: { net: "main" }, a: 0 }, rev: 7 });
const ok = (body) => ({ config: body.config, rev: body.rev + 1 });

describe("saveConfigFields on unknown_net", () => {
  it("re-reads the config and the second PUT succeeds", async () => {
    const w = setup([unknown, fresh, ok]);
    const res = await saveConfigFields(w.store, { a: 1 }, w.deps);
    assert.deepEqual(res, { ok: true });
    assert.deepEqual(w.calls.map((c) => `${c.method} ${c.path}`), ["PUT /api/config", "GET /api/config", "PUT /api/config"]);
    assert.deepEqual(w.calls[2].body, { rev: 7, config: { proxy: { net: "main" }, a: 1 } });
  });

  it("a second unknown_net is returned: two PUTs, one GET, no events", async () => {
    const w = setup([unknown, fresh, unknown]);
    const res = await saveConfigFields(w.store, { a: 1 }, w.deps);
    assert.equal(res.ok, false);
    assert.equal(res.error.code, "unknown_net");
    assert.deepEqual(w.calls.map((c) => c.method), ["PUT", "GET", "PUT"]);
    assert.deepEqual(w.events, []);
  });

  it("another refusal is not retried", async () => {
    for (const [status, code] of [[500, "internal"], [422, "invalid"]]) {
      const w = setup([() => { throw new ApiError(status, code, "refused"); }]);
      const res = await saveConfigFields(w.store, { a: 1 }, w.deps);
      assert.equal(res.ok, false);
      assert.equal(res.error.code, code);
      assert.deepEqual(w.calls.map((c) => c.method), ["PUT"]);
    }
  });
});
