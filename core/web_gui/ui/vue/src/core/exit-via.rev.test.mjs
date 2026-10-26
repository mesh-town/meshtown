/* After an exit is written into another network the server has already rewritten the primary config
 * (the proxy moved there): the page must hold the new revision, or its next save is refused as stale. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { NET_B } from "../testing/overview-fixtures.mjs";
import { ApiError } from "./api.ts";
import { saveConfigFields } from "./save-config.ts";
import { saveExitIn } from "./exit-via.ts";
import { createStore } from "./store.ts";

const MAIN_REV = 5;

const brief = (id, over = {}) => ({
  id, label: id, id_hex: "hex-" + id, status: "running", exit_enabled: false, proxy: false, route_state: "direct", exits: [],
  joining: false, members_admitting: 0, me_excluded: false, me_pending: false, peers_online: 0, peers_total: 0, ...over,
});

// `bump` — how far a write into network B moves the revision of the primary config on the server.
function setup({ bump = 2, failSlot = null } = {}) {
  const store = createStore();
  store.config = { route: { exits: [] } };
  store.configRev = MAIN_REV;
  store.nets = [brief("main", { proxy: true }), brief(NET_B)];
  for (const b of store.nets) {
    store.slots[b.id] = { id: b.id, brief: b, state: { boot_id: "boot-" + b.id, config_rev: 1, route: { state: "direct", exits: [] }, peers: [] }, members: null, config: { route: { exits: [] } }, configRev: 11, configMeta: null, bootId: null };
  }
  store.lead = "main";
  store.state = store.slots.main.state;
  const server = { mainRev: MAIN_REV, stale: 0, paths: [] };
  const api = async (method, path, body) => {
    server.paths.push(`${method} ${path}`);
    if (method === "GET") return { config: { route: { exits: [] } }, rev: server.mainRev };
    if (path.startsWith("/api/net/")) {
      if (failSlot) throw failSlot;
      server.mainRev += bump;
      return { config: body.config, rev: body.rev + 1 };
    }
    if (body.rev !== server.mainRev) { server.stale += 1; throw new ApiError(409, "stale_rev", "stale"); }
    server.mainRev += 1;
    return { config: body.config, rev: server.mainRev };
  };
  return { store, server, deps: { api, busTarget: new EventTarget(), remember: () => {} } };
}

describe("saveExitIn and the revision of the primary config", () => {
  it("after a write into B the next main save carries the new rev and has no 409", async () => {
    const w = setup();
    await saveExitIn(w.store, NET_B, "BB", w.deps);
    assert.equal(w.store.configRev, w.server.mainRev);
    const res = await saveConfigFields(w.store, { "node.name": "x" }, w.deps);
    assert.equal(res.ok, true);
    assert.equal(w.server.stale, 0);
  });

  it("a write into main does not read the main config twice", async () => {
    const w = setup();
    await saveExitIn(w.store, "main", "BB", w.deps);
    assert.deepEqual(w.server.paths, ["PUT /api/config"]);
  });

  it("a refused write leaves the main config unread", async () => {
    const w = setup({ failSlot: new ApiError(422, "invalid", "refused") });
    const st = await saveExitIn(w.store, NET_B, "BB", w.deps);
    assert.equal(st.cls, "field-error-text");
    assert.deepEqual(w.server.paths, [`PUT /api/net/${NET_B}/config`]);
  });
});
