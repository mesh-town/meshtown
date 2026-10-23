/* Self-test of the fake node: the helper the moved-over cases rely on is itself checked. */

import { describe, expect, it } from "vitest";
import { demoConfig, demoState } from "./demo-node.mjs";
import { createFakeNode } from "./fake-node.mjs";
import { membersReply } from "./members-fixtures.mjs";

const make = () => createFakeNode({ state: demoState(), config: demoConfig() });
const json = (res) => res.json();

describe("fake-node", () => {
  it("the snapshot is a new object each time, with config_rev", async () => {
    const node = make();
    const a = await json(await node.fetch("/api/state"));
    const b = await json(await node.fetch("/api/state"));
    expect(a).toEqual(b);
    expect(a).not.toBe(b);
    expect(a.config_rev).toBe(1);
  });

  it("PUT on a stale rev gives 409 stale_rev with the current document; on the right one, rev + 1", async () => {
    const node = make();
    node.rev = 3;
    const stale = await node.fetch("/api/config", { method: "PUT", body: JSON.stringify({ rev: 2, config: {} }) });
    expect(stale.status).toBe(409);
    expect(await json(stale)).toMatchObject({ error: "stale_rev", rev: 3, config: node.config });
    const ok = await node.fetch("/api/config", { method: "PUT", body: JSON.stringify({ rev: 3, config: { route: { exits: [] } } }) });
    expect(ok.status).toBe(200);
    expect(node.rev).toBe(4);
  });

  it('fail = "down" — fetch throws TypeError', async () => {
    const node = make();
    node.fail = "down";
    await expect(node.fetch("/api/state")).rejects.toBeInstanceOf(TypeError);
  });

  it("an unknown path gives 404 and an entry in unexpected", async () => {
    const node = make();
    const res = await node.fetch("/api/nope");
    expect(res.status).toBe(404);
    expect(node.unexpected).toEqual(["GET /api/nope"]);
  });
});

const SLOT_B = "k7q2xa";
const post = (node, path, body) => node.fetch(path, { method: "POST", body: JSON.stringify(body) });
const get = async (node, path) => json(await node.fetch(path));

describe("fake-node > overview", () => {
  describe("without slots", () => {
    it("the overview is one `main` slot: the state of `/api/state`, the network label in `nets`", async () => {
      const node = setup();
      const ov = await get(node, "/api/overview");
      expect(ov.api).toBe(2);
      expect(ov.nets.map((n) => [n.id, n.label])).toEqual([["main", "demo-network"]]);
      expect(ov.slots.map((s) => s.slot)).toEqual(["main"]);
      expect(ov.slots[0].state).toEqual(await get(node, "/api/state"));
      expect(ov.restart_slots).toEqual([]);
    });

    it("members come from `routes[GET /api/members]`; without a route the slot has `members: null`", async () => {
      const bare = setup();
      expect((await get(bare, "/api/overview")).slots[0].members).toBeNull();
      const reply = membersReply();
      const withList = setup({ routes: { "GET /api/members": reply } });
      expect((await get(withList, "/api/overview")).slots[0].members).toEqual(reply);
    });

    it("the overview request is recorded in `calls` and is not an unexpected request", async () => {
      const node = setup();
      await get(node, "/api/overview");
      expect(node.count("GET", "/api/overview")).toBe(1);
      expect(node.unexpected).toEqual([]);
    });

    it("an overridden `routes[GET /api/state]` is the state inside the overview", async () => {
      const over = { ...demoState(), boot_id: "overridden-boot" };
      const node = setup({ routes: { "GET /api/state": () => over } });
      const ov = await get(node, "/api/overview");
      expect(ov.slots[0].state.boot_id).toBe("overridden-boot");
    });

    it("`fail.path = /api/state` fails the overview request with the same reply", async () => {
      const node = setup();
      node.fail = { path: "/api/state", status: 503, body: { error: "busy", message: "starting" } };
      const res = await node.fetch("/api/overview");
      expect(res.status).toBe(503);
      expect(await json(res)).toEqual({ error: "busy", message: "starting" });
    });

    it("`fail = down` makes the overview request throw, as every other request", async () => {
      const node = setup();
      node.fail = "down";
      await expect(node.fetch("/api/overview")).rejects.toBeInstanceOf(TypeError);
    });
  });

  describe("legacy: true", () => {
    it("the overview answers 404 not_found, `/api/state` still works", async () => {
      const node = setup({ legacy: true });
      const res = await node.fetch("/api/overview");
      expect(res.status).toBe(404);
      expect(await json(res)).toMatchObject({ error: "not_found" });
      expect((await node.fetch("/api/state")).status).toBe(200);
    });

    it("the probe is not an unexpected request: an older node is a supported scenario", async () => {
      const node = setup({ legacy: true });
      await node.fetch("/api/overview");
      expect(node.unexpected).toEqual([]);
    });
  });

  describe("with slots", () => {
    it("the overview lists every slot in order, with its own state and members", async () => {
      const node = setup(twoSlots());
      const ov = await get(node, "/api/overview");
      expect(ov.slots.map((s) => s.slot)).toEqual(["main", SLOT_B]);
      expect(ov.nets.map((n) => [n.id, n.label])).toEqual([["main", "friends"], [SLOT_B, "my devices"]]);
      expect(ov.slots[1].state.network.label).toBe("my devices");
      expect(ov.slots[1].members.members.length, "the members of slot B, not of main").toBe(1);
    });

    it("`/api/<tail>` answers from main, `/api/net/<id>/<tail>` from that slot", async () => {
      const node = setup(twoSlots());
      expect((await get(node, "/api/state")).network.label).toBe("friends");
      expect((await get(node, `/api/net/${SLOT_B}/state`)).network.label).toBe("my devices");
      expect((await get(node, "/api/members")).members.length).toBe(3);
      expect((await get(node, `/api/net/${SLOT_B}/members`)).members.length).toBe(1);
    });

    it("the config of a slot is its own: a write to slot B leaves the document of main alone", async () => {
      const node = setup(twoSlots());
      const before = await get(node, "/api/config");
      const slotCfg = await get(node, `/api/net/${SLOT_B}/config`);
      const res = await node.fetch(`/api/net/${SLOT_B}/config`, {
        method: "PUT", body: JSON.stringify({ rev: slotCfg.rev, config: { route: { exits: [] } } }),
      });
      expect(res.status).toBe(200);
      expect(await get(node, "/api/config")).toEqual(before);
      expect(node.unexpected).toEqual([]);
    });

    it("`POST /api/nets` answers 201 with a new slot id and the list of nets, and the slot shows in the overview", async () => {
      const node = setup(twoSlots());
      const res = await post(node, "/api/nets", {});
      expect(res.status).toBe(201);
      const body = await json(res);
      expect(body.slot).toMatch(/^[a-z2-7]{6}$/);
      expect(body.nets.map((n) => n.id)).toEqual(["main", SLOT_B, body.slot]);
      const ov = await get(node, "/api/overview");
      expect(ov.slots.map((s) => s.slot)).toEqual(["main", SLOT_B, body.slot]);
    });

    it("`members/leave` of slot B answers left, announced and the remaining nets, and the slot is gone", async () => {
      const node = setup(twoSlots());
      const res = await post(node, `/api/net/${SLOT_B}/members/leave`, { confirm: true });
      expect(res.status).toBe(200);
      const body = await json(res);
      expect(body.left).toBe(true);
      expect(typeof body.announced, "announced is a boolean").toBe("boolean");
      expect(body.nets.map((n) => n.id)).toEqual(["main"]);
      const ov = await get(node, "/api/overview");
      expect(ov.slots.map((s) => s.slot)).toEqual(["main"]);
      expect(ov.nets.map((n) => n.id)).toEqual(["main"]);
    });

    it("a request to a slot that has left answers 404 unknown_net", async () => {
      const node = setup(twoSlots());
      await post(node, `/api/net/${SLOT_B}/members/leave`, { confirm: true });
      const res = await node.fetch(`/api/net/${SLOT_B}/state`);
      expect(res.status).toBe(404);
      expect(await json(res)).toMatchObject({ error: "unknown_net" });
    });
  });
});

// A node of one network (the demo state), with options for `createFakeNode` on top.
function setup(over = {}) {
  return createFakeNode({ state: demoState(), config: demoConfig(), ...over });
}

// Two slots: `main` ("friends", three members) and slot B ("my devices", one member).
function twoSlots() {
  const label = (name, id_hex) => ({ label: name, id_hex, masked: name + "--••••••••" });
  const mainState = demoState({ slot: "main", network: label("friends", "a1b2c3d4") });
  const bState = demoState({ slot: SLOT_B, network: label("my devices", "k7q2xa00") });
  const bMembers = membersReply({ rows: [membersReply().members[0]] });
  return {
    state: mainState,
    slots: [
      { id: "main", state: mainState, members: membersReply(), config: demoConfig() },
      { id: SLOT_B, state: bState, members: bMembers, config: demoConfig() },
    ],
  };
}
