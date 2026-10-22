/* Self-check of the fake node: writing `socks.ifaces`, `socks.access` and `web.ifaces`, the pending restart and
 * `POST /api/actions/restart` (`fake-node.mjs`, `fake-node-config.mjs`). If the fake node lies, the network selection tests lie too. */

import { describe, expect, it } from "vitest";
import { createFakeNode } from "./fake-node.mjs";
import { laptopState, netsConfig, routerState } from "./nets-states.mjs";

describe("fake-node > networks and restart", () => {
  describe("socks.ifaces and socks.access: take effect after a restart", () => {
    it("PUT of a new list: changed and restart_pending name the field, the snapshot is still the old one", async () => {
      const node = setup(routerState());
      const res = await put(node, { "socks.ifaces": ["br-lan", "eth0"], "web.ifaces": ["br-lan", "eth0"] });
      expect(res.status).toBe(200);
      expect(res.body.changed).toEqual(["socks.ifaces"]);
      expect(res.body.restart_pending).toEqual(["socks.ifaces"]);
      expect(res.body.restart_fields).toContain("socks.ifaces");
      expect(node.state.node.networks.find((n) => n.name === "eth0").open, "eth0 is still closed").toBe(false);
      expect((await get(node, "/api/config")).restart_pending).toEqual(["socks.ifaces"]);
    });

    it("returning the previous value clears the pending state", async () => {
      const node = setup(routerState());
      await put(node, { "socks.ifaces": ["br-lan", "eth0"] });
      const back = await put(node, { "socks.ifaces": [] });
      expect(back.body.restart_pending).toEqual([]);
    });

    it("socks.access: a change goes into pending", async () => {
      const node = setup(routerState());
      const res = await put(node, { "socks.access": "localhost" });
      expect(res.body.changed).toEqual(["socks.access"]);
      expect(res.body.restart_pending).toEqual(["socks.access"]);
    });

    it("an unrelated edit (port) gives neither changed nor pending", async () => {
      const node = setup(routerState());
      const res = await put(node, { "socks.port": 1081 });
      expect(res.body.changed).toEqual([]);
      expect(res.body.restart_pending).toEqual([]);
    });

    it("POST /api/actions/restart: 202, pending cleared, new boot_id, networks and socks.lan recomputed from the config", async () => {
      const node = setup(routerState());
      const before = node.state.boot_id;
      await put(node, { "socks.ifaces": ["br-lan", "eth0"], "web.ifaces": ["br-lan", "eth0"] });
      const res = await post(node, "/api/actions/restart");
      expect(res.status).toBe(202);
      expect((await get(node, "/api/config")).restart_pending).toEqual([]);
      const eth0 = node.state.node.networks.find((n) => n.name === "eth0");
      expect(eth0).toMatchObject({ open: true, by: "list" });
      expect(node.state.socks.ifaces).toEqual(["br-lan", "eth0"]);
      expect(node.state.socks.lan.map((l) => l.iface)).toEqual(["br-lan", "eth0"]);
      expect(node.state.boot_id).not.toBe(before);
      expect(node.unexpected).toEqual([]);
    });

    it("after restart with an empty list it is automatic again: socks.ifaces null, eth0 closed by auto", async () => {
      const node = setup(routerState({ ifaces: ["br-lan", "eth0"] }));
      await put(node, { "socks.ifaces": [], "web.ifaces": [] });
      await post(node, "/api/actions/restart");
      expect(node.state.socks.ifaces).toBe(null);
      expect(node.state.node.networks.find((n) => n.name === "eth0")).toMatchObject({ open: false, by: "auto" });
    });

    it("socks.access=localhost after restart: networks closed by none, socks.lan empty", async () => {
      const node = setup(routerState());
      await put(node, { "socks.access": "localhost" });
      await post(node, "/api/actions/restart");
      expect(node.state.socks.lan).toEqual([]);
      expect(node.state.node.networks.every((n) => !n.open && n.by === "none")).toBe(true);
    });
  });

  describe("web.ifaces: takes effect at once and does not wait for a restart", () => {
    it("PUT of a list: web.ifaces in the snapshot is already new, it is not in restart_pending", async () => {
      const node = setup(routerState());
      const res = await put(node, { "web.ifaces": ["br-lan", "eth0"] });
      expect(node.state.web.ifaces).toEqual(["br-lan", "eth0"]);
      expect(res.body.restart_pending).toEqual([]);
    });

    it("an empty list gives null in the snapshot (automatic)", async () => {
      const node = setup(routerState({ ifaces: ["br-lan"] }));
      await put(node, { "web.ifaces": [] });
      expect(node.state.web.ifaces).toBe(null);
    });
  });

  describe("web.ifaces, that closes the current connection: 422", () => {
    it("a list without via_iface gives 422 invalid with the web.ifaces field, config and revision unchanged", async () => {
      const node = setup(routerState());
      const rev = node.rev;
      const res = await put(node, { "web.ifaces": ["eth0"] });
      expect(res.status).toBe(422);
      expect(res.body.error).toBe("invalid");
      expect(res.body.fields[0].path).toBe("web.ifaces");
      expect(typeof res.body.fields[0].message).toBe("string");
      expect(node.rev).toBe(rev);
      expect(node.config.web.ifaces).toEqual([]);
    });

    it('["*"] and a list with via_iface pass', async () => {
      const node = setup(routerState());
      expect((await put(node, { "web.ifaces": ["*"] })).status).toBe(200);
      expect((await put(node, { "web.ifaces": ["br-lan"] })).status).toBe(200);
    });

    it("an empty list (automatic) on a router: passes from the home network, 422 from the ISP network", async () => {
      expect((await put(setup(routerState()), { "web.ifaces": [] })).status).toBe(200);
      const fromProvider = setup(routerState({ node: { ...routerState().node, via_iface: "eth0" } }));
      expect((await put(fromProvider, { "web.ifaces": [] })).status).toBe(422);
    });

    it("via_iface null (page opened on the machine itself): any list passes", async () => {
      const node = setup(laptopState({ access: "localnet" }));
      expect((await put(node, { "web.ifaces": ["utun3"] })).status).toBe(200);
    });
  });
});

function setup(state) {
  return createFakeNode({ state, config: netsConfig(state) });
}

async function call(node, method, path, body) {
  const res = await node.fetch(path, { method, body: body ? JSON.stringify(body) : undefined });
  return { status: res.status, body: await res.json() };
}

const get = async (node, path) => (await call(node, "GET", path)).body;
const post = (node, path) => call(node, "POST", path);

// An edit by flat paths on top of the node's current config: `{ "socks.ifaces": [...] }`.
function put(node, changes) {
  const config = structuredClone(node.config);
  for (const [path, value] of Object.entries(changes)) {
    const keys = path.split(".");
    const last = keys.pop();
    keys.reduce((o, k) => (o[k] ??= {}), config)[last] = value;
  }
  return call(node, "PUT", "/api/config", { rev: node.rev, config });
}
