/* Picking an exit on Home with a stale copy of the config: the server refuses a PUT that still names a network
 * the node has left, and the choice must still reach a 200. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "./api.ts";
import { saveDirect, saveExitIn } from "./exit-via.ts";
import { createStore } from "./store.ts";

const FRESH = { route: { exits: [] }, proxy: { net: "main" } };

function setup() {
  const store = createStore();
  store.config = { route: { exits: [] }, proxy: { net: "gone" } };
  store.configRev = 5;
  store.nets = [{ id: "main", label: "main", id_hex: "hex-main", status: "online", exit_enabled: false, proxy: true, route_state: "direct", exits: [], members_voting: 1, joining: false, members_admitting: 0, me_excluded: false, me_pending: false, peers_online: 0, peers_total: 0 }];
  store.lead = "main";
  const server = { puts: 0, gets: 0 };
  const api = async (method, path, body) => {
    if (method === "GET") { server.gets += 1; return { config: FRESH, rev: 9 }; }
    server.puts += 1;
    if (body.config.proxy.net !== "main") throw new ApiError(404, "unknown_net", "unknown network");
    return { config: body.config, rev: body.rev + 1 };
  };
  return { store, server, deps: { api, busTarget: new EventTarget(), remember: () => {} } };
}

describe("exit choice against a stale proxy.net", () => {
  it("saveExitIn reaches a 200 after one re-read", async () => {
    const w = setup();
    const st = await saveExitIn(w.store, "main", "BB", w.deps);
    assert.equal(st.cls, "saved");
    assert.equal(w.server.puts, 2);
    assert.equal(w.server.gets, 1);
  });

  it("saveDirect reaches a 200 after one re-read", async () => {
    const w = setup();
    const st = await saveDirect(w.store, w.deps);
    assert.equal(st.cls, "saved");
    assert.equal(w.server.puts, 2);
  });
});
