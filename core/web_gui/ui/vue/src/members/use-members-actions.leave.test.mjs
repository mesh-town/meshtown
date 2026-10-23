/* `leaveNet` and the leave flow of the Members tab (`useMembersActions`) across several networks. */

import { describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { ApiError, api as coreApi } from "../core/api.ts";
import { leaveNet, parseLeave, postMemberAction } from "./members-api.ts";
import { useMembersActions } from "./use-members-actions.ts";
import { acceptOverview, parseOverview } from "../core/overview.ts";
import { createStore } from "../core/store.ts";
import { createFakeNode } from "../testing/fake-node.mjs";
import { NET_A, NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { SELF_FULL } from "../testing/section.mjs";
import { membersReply, peerOneRow, selfRow, EXTRA_FULL } from "../testing/members-fixtures.mjs";

const plain = (value) => JSON.parse(JSON.stringify(value));

describe("leaveNet", () => {
  const answer = (over = {}) => ({ ...membersReply(), left: true, announced: true, nets: [briefOf(NET_A, "friends")], ...over });

  it("another network: one POST /api/net/<slot>/members/leave with { confirm: true }, nothing else", async () => {
    const { api, call } = setup({ reply: answer() });
    const result = await leaveNet(api, NET_B);
    assert.equal(api.mock.calls.length, 1);
    assert.deepEqual(call(0), { method: "POST", path: "/api/net/" + NET_B + "/members/leave", body: { confirm: true } });
    assert.equal(result.left, true);
    assert.deepEqual(result.nets.map((n) => n.id), [NET_A]);
  });

  it("the main network: POST /api/members/leave", async () => {
    const { api, call } = setup({ reply: answer() });
    await leaveNet(api, NET_A);
    assert.deepEqual(call(0), { method: "POST", path: "/api/members/leave", body: { confirm: true } });
  });

  it("`announced: false` of the reply is passed on", async () => {
    const { api } = setup({ reply: answer({ announced: false }) });
    assert.equal((await leaveNet(api, NET_B)).announced, false);
  });

  it("an api error is not swallowed", async () => {
    const { api } = setup({ fail: new ApiError(409, "busy") });
    await assert.rejects(leaveNet(api, NET_B), (err) => err.status === 409 && err.code === "busy");
  });

  it("a reply that is not a member list rejects with an Error", async () => {
    const { api } = setup({ reply: { ok: true } });
    await assert.rejects(leaveNet(api, NET_B), Error);
  });
});

describe("leaving a network from the Members tab (useMembersActions)", () => {
  describe("two networks, leave in B", () => {
    it("sends one POST /api/net/<B>/members/leave { confirm: true } and no DELETE", async () => {
      const env = await setupPage(twoNets());
      await env.leave(NET_B);
      assert.deepEqual(env.writes(), [{ method: "POST", path: "/api/net/" + NET_B + "/members/leave", body: { confirm: true } }]);
    });

    it("B is gone from the store and A is selected", async () => {
      const env = await setupPage(twoNets());
      await env.leave(NET_B);
      assert.equal(env.store.slots[NET_B], undefined, "the slice of B must be dropped");
      assert.deepEqual(env.store.nets.map((n) => n.id), [NET_A]);
      assert.equal(env.store.lead, NET_A);
      assert.equal(env.store.peersTab, NET_A);
    });

    it("A keeps its slice and its member list", async () => {
      const env = await setupPage(twoNets());
      await env.leave(NET_B);
      assert.ok(env.store.slots[NET_A].members, "main members must stay");
      assert.equal(env.store.members, env.store.slots[NET_A].members);
    });

    it("the page is woken (`state` on the bus) and the line under the list is set", async () => {
      const env = await setupPage(twoNets());
      await env.leave(NET_B);
      assert.deepEqual(env.bus, ["state"]);
      assert.notEqual(env.actions.status.value.text, "");
      assert.equal(env.actions.status.value.cls, "");
    });
  });

  describe("A (main) gives up its key while B stays", () => {
    it("the slice of main has no members and B leads", async () => {
      const keyless = { ...briefOf(NET_A, null), id_hex: null };
      const env = await setupPage(twoNets(), { leaveAnswer: { ...membersReply(), left: true, announced: true, nets: [keyless, briefOf(NET_B, "my devices")] } });
      await env.leave(NET_A);
      assert.deepEqual(env.writes().map((w) => w.path), ["/api/members/leave"]);
      assert.equal(env.store.slots[NET_A].members, null);
      assert.equal(env.store.lead, NET_B);
      assert.equal(env.store.peersTab, NET_B);
    });
  });

  describe("the node did not leave (left: false)", () => {
    it("the slice stays, the briefs of the reply replace `nets`", async () => {
      const nets = [briefOf(NET_A, "friends"), briefOf(NET_B, "my devices", { members_voting: 1 })];
      const env = await setupPage(twoNets(), { leaveAnswer: { ...membersReply(), left: false, announced: false, nets } });
      await env.leave(NET_B);
      assert.ok(env.store.slots[NET_B], "the slice of B must stay");
      assert.equal(env.store.nets[1].members_voting, 1);
      assert.deepEqual(env.bus, ["state"]);
    });

    it("an empty `nets` leaves the old briefs", async () => {
      const env = await setupPage(twoNets(), { leaveAnswer: { ...membersReply(), left: false, announced: false, nets: [] } });
      const before = env.store.nets;
      await env.leave(NET_B);
      assert.equal(env.store.nets, before);
      assert.ok(env.store.slots[NET_B]);
    });
  });

  describe("the slot is the lead by default", () => {
    it("with no slot given, leave goes to the lead network", async () => {
      const keyless = { ...briefOf(NET_A, null), id_hex: null };
      const env = await setupPage(twoNets());
      acceptOverview(env.store, withNets(env.ov, [keyless, briefOf(NET_B, "my devices")]));
      assert.equal(env.store.lead, NET_B);
      await env.leaveByDefault();
      assert.deepEqual(env.writes().map((w) => w.path), ["/api/net/" + NET_B + "/members/leave"]);
    });
  });

  describe("another action of a network is written to its slice", () => {
    it("block in B: POST /api/net/<B>/members/block, the answer goes to B and not to the lead", async () => {
      const reply = membersReply({ rows: [selfRow(), peerOneRow({ blocked_by_me: true })] });
      const env = await setupPage(twoNets(), { actionAnswer: { ...reply, changed: true } });
      const leadMembers = env.store.members;
      await env.actions.run("block", EXTRA_FULL);
      assert.deepEqual(env.writes().map((w) => w.path), ["/api/net/" + NET_B + "/members/block"]);
      assert.equal(env.store.slots[NET_B].members.members[1].blocked_by_me, true);
      assert.equal(env.store.members, leadMembers, "the lead (A) must not get the list of B");
    });
  });

  describe("the tab changes while the dialog is open", () => {
    it("the action goes to the network named in the dialog", async () => {
      const env = await setupPage(twoNets(), { actionAnswer: { ...membersReply(), changed: true } });
      let current = NET_B;
      const shifting = useMembersActions(env.store, () => current, { api: env.call, busTarget: new EventTarget() });
      shifting.dialog.value = {
        ask: async () => {
          current = NET_A;
          return true;
        },
      };
      await shifting.ask("block", EXTRA_FULL);
      assert.deepEqual(env.writes().map((w) => w.path), ["/api/net/" + NET_B + "/members/block"]);
    });

    it("leave goes to the network named in the dialog", async () => {
      const env = await setupPage(twoNets());
      let current = NET_B;
      const shifting = useMembersActions(env.store, () => current, { api: env.call, busTarget: new EventTarget() });
      shifting.dialog.value = {
        ask: async () => {
          current = NET_A;
          return true;
        },
      };
      await shifting.ask("leave", SELF_FULL);
      assert.deepEqual(env.writes().map((w) => w.path), ["/api/net/" + NET_B + "/members/leave"]);
    });
  });

  describe("one network", () => {
    it("leave goes to /api/members/leave and main stays the lead after the next poll", async () => {
      const one = { ...twoNets(), slots: [twoNets().slots[0]] };
      const env = await setupPage(one, { slot: NET_A });
      await env.leave(NET_A);
      assert.deepEqual(env.writes().map((w) => w.path), ["/api/members/leave"]);
      await env.poll();
      assert.equal(env.store.lead, NET_A);
      assert.equal(env.store.peersTab, NET_A);
      assert.deepEqual(env.store.nets.map((n) => n.id), [NET_A]);
    });
  });
});

describe("postMemberAction at an address of a network", () => {
  describe("another network", () => {
    it.each(["exclude", "unexclude", "block", "unblock"])("%s: POST /api/net/<slot>/members/%s with the id", async (action) => {
      const { api, call } = setup({ reply: { ...membersReply(), changed: true } });
      await postMemberAction(api, action, EXTRA_FULL, undefined, NET_B);
      assert.deepEqual(call(0), { method: "POST", path: "/api/net/" + NET_B + "/members/" + action, body: { id: EXTRA_FULL } });
    });

    it("exclude with a reason: the address moves, the body is still { id, reason }", async () => {
      const { api, call } = setup({ reply: { ...membersReply(), changed: true } });
      await postMemberAction(api, "exclude", EXTRA_FULL, "lost laptop", NET_B);
      assert.deepEqual(call(0), {
        method: "POST",
        path: "/api/net/" + NET_B + "/members/exclude",
        body: { id: EXTRA_FULL, reason: "lost laptop" },
      });
    });

    it("leave: POST /api/net/<slot>/members/leave with { confirm: true }", async () => {
      const { api, call } = setup({ reply: { ...membersReply(), changed: true } });
      await postMemberAction(api, "leave", undefined, undefined, NET_B);
      assert.deepEqual(call(0), { method: "POST", path: "/api/net/" + NET_B + "/members/leave", body: { confirm: true } });
    });
  });

  describe("the main network or no slot (one network)", () => {
    it("the slot `main` keeps the old address", async () => {
      const { api, call } = setup({ reply: { ...membersReply(), changed: true } });
      await postMemberAction(api, "exclude", EXTRA_FULL, undefined, NET_A);
      assert.equal(call(0).path, "/api/members/exclude");
    });

    it("no slot keeps the old address", async () => {
      const { api, call } = setup({ reply: { ...membersReply(), changed: true } });
      await postMemberAction(api, "block", EXTRA_FULL);
      assert.equal(call(0).path, "/api/members/block");
    });
  });
});

describe("parseLeave", () => {
  const briefs = () => [briefOf(NET_A, "friends"), briefOf(NET_B, "my devices")];

  describe("a full reply", () => {
    it("keeps `left`, `announced` and `nets`, and the member list", () => {
      const raw = { ...membersReply(), left: true, announced: false, nets: briefs() };
      const parsed = parseLeave(raw);
      assert.equal(parsed.left, true);
      assert.equal(parsed.announced, false);
      assert.deepEqual(parsed.nets.map((n) => n.id), [NET_A, NET_B]);
      assert.deepEqual(parsed.members.map((r) => r.id_full), membersReply().members.map((r) => r.id_full));
    });

    it("`left: false` is kept", () => {
      assert.equal(parseLeave({ ...membersReply(), left: false, announced: false, nets: briefs() }).left, false);
    });
  });

  describe("an older node (a plain member list)", () => {
    it("`left` and `announced` are true, `nets` is empty", () => {
      const parsed = parseLeave(membersReply());
      assert.equal(parsed.left, true);
      assert.equal(parsed.announced, true);
      assert.deepEqual(plain(parsed.nets), []);
    });

    it("`nets` that is not a list reads as empty", () => {
      assert.deepEqual(plain(parseLeave({ ...membersReply(), nets: "x" }).nets), []);
    });
  });

  describe("garbage", () => {
    it.each([
      ["null", null],
      ["a string", "left"],
      ["an array", []],
      ["an object without members", { left: true, announced: true, nets: [] }],
      ["members is not an array", { members: "x", left: true }],
    ])("%s -> null", (_name, raw) => {
      assert.equal(parseLeave(raw), null);
    });
  });
});

// ---------------------------------------------------------------- setup

// A fake `api`: answers every call with `reply` (or rejects with `fail`) and records the calls.
function setup({ reply, fail } = {}) {
  const api = vi.fn(async () => {
    if (fail) throw fail;
    return reply;
  });
  const call = (index) => {
    const [method, path, body] = api.mock.calls[index];
    return { method, path, body };
  };
  return { api, call };
}

// A brief of a network with a key as the overview gives it; `label` null — no label.
function briefOf(id, label, over = {}) {
  return {
    id, label, id_hex: id === NET_A ? "net-aaaa" : "net-bbbb", status: "online", exit_enabled: false, proxy: id === NET_A,
    route_state: "direct", exits: [], members_voting: 0, joining: false, members_admitting: 0, me_excluded: false, me_pending: false,
    peers_online: 1, peers_total: 2, ...over,
  };
}

const withNets = (ov, nets) => ({ ...ov, nets });

// The Members tab on a store filled from a fake node's overview. `slot` is the network of the tab (B by default). The node answers
// the overview itself; a `leaveAnswer` / `actionAnswer` replaces its answer to the leave / block POST. `writes()` lists the POSTs and DELETEs
// that were sent, `bus` the words sent on the bus.
async function setupPage(nodeOptions, { slot = NET_B, leaveAnswer = null, actionAnswer = null } = {}) {
  const node = createFakeNode(nodeOptions);
  const fetchOnNode = (input, init) => node.fetch(input, init);
  const api = (method, path, body) => coreApi(method, path, body, { fetch: fetchOnNode, getToken: () => "t", timeoutMs: 1000, busTarget: undefined });
  const answers = {
    "POST /api/members/leave": leaveAnswer,
    [`POST /api/net/${slot}/members/leave`]: leaveAnswer,
    [`POST /api/net/${slot}/members/block`]: actionAnswer,
  };
  const call = async (method, path, body) => {
    const fixed = answers[method + " " + path];
    if (fixed) {
      node.calls.push({ method, path, body });
      return fixed;
    }
    return api(method, path, body);
  };
  const poll = async () => acceptOverview(store, parseOverview(await call("GET", "/api/overview")));
  const store = createStore();
  const ov = parseOverview(await call("GET", "/api/overview"));
  acceptOverview(store, ov);
  const bus = [];
  const busTarget = new EventTarget();
  busTarget.addEventListener("meshtown:changed", (e) => bus.push(e.detail.what));
  const actions = useMembersActions(store, () => slot, { api: call, busTarget });
  const byDefault = useMembersActions(store, undefined, { api: call, busTarget });
  const writes = () => node.calls.filter((c) => c.method !== "GET");
  return {
    node, store, ov, bus, actions, writes, poll, call,
    leave: async (target) => {
      slot = target;
      await actions.run("leave", SELF_FULL);
    },
    leaveByDefault: () => byDefault.run("leave", SELF_FULL),
  };
}
