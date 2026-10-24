/* The removal actions of the Members tab (`useMembersActions`): which dialog `forget` opens, which network the request
 * goes to, the withdraw without a dialog, the pending-ban warning and the line under the list. */

import { describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { api as coreApi } from "../core/api.ts";
import { acceptOverview, parseOverview, putSlotMembers } from "../core/overview.ts";
import { createStore } from "../core/store.ts";
import { createFakeNode } from "../testing/fake-node.mjs";
import { NET_A, NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { AT_UNIX, actionReply, foreignVote, forgetInfo, membersReply, peerOneRow, selfRow, staleRow } from "../testing/members-fixtures.mjs";
import { useMembersActions } from "./use-members-actions.ts";

const NINE_DAYS = 9 * 86400;
const OLD = staleRow("old-desktop", NINE_DAYS);
const MY_CIRCLE = forgetInfo({ mine: true, proposed_by: "aaaa1111", voters: ["aaaa1111"] });
const ABOUT_ADDRESS = (slot, action) => "/api/net/" + slot + "/members/" + action;

describe("picking Remove from network", () => {
  describe("no circle yet", () => {
    it("pick_forget_asks_by_need: two members are enough, so the dialog removes right away", async () => {
      const env = await setup({ members: membersReply({ rows: [selfRow(), peerOneRow(), OLD], need: 1 }) });
      await env.pick("forget", OLD.id_full);
      assert.equal(env.asked.length, 1);
      assert.equal(env.asked[0].action, "Remove old-desktop");
      assert.match(env.asked[0].body.join(" "), /You and peer-one are the only members/);
    });

    it("pick_forget_asks_by_need: two votes are needed, so the dialog only proposes", async () => {
      const env = await setup({ members: membersReply({ rows: [selfRow(), peerOneRow(), OLD], need: 2 }) });
      await env.pick("forget", OLD.id_full);
      assert.equal(env.asked[0].action, "Propose removal");
      assert.equal(env.asked[0].title, "Remove old-desktop from the network?");
    });
  });

  describe("the circle is someone else's", () => {
    it("pick_forget_asks_by_need: the dialog asks to confirm", async () => {
      const waiting = { ...OLD, forget: forgetInfo({ mine: false }) };
      const env = await setup({ members: membersReply({ rows: [selfRow(), peerOneRow(), waiting], need: 2 }) });
      await env.pick("forget", OLD.id_full);
      assert.equal(env.asked[0].title, "Confirm removing old-desktop?");
      assert.equal(env.asked[0].action, "Remove old-desktop");
    });

    it("pick_forget_asks_by_need: a one-vote threshold does not turn it back into a plain removal", async () => {
      const waiting = { ...OLD, forget: forgetInfo({ mine: false }) };
      const env = await setup({ members: membersReply({ rows: [selfRow(), peerOneRow(), waiting], need: 1 }) });
      await env.pick("forget", OLD.id_full);
      assert.equal(env.asked[0].title, "Confirm removing old-desktop?");
    });
  });

  describe("what the dialog says about the target", () => {
    it("says how long ago it was last online", async () => {
      const env = await setup({ members: membersReply({ rows: [selfRow(), peerOneRow(), OLD] }) });
      await env.pick("forget", OLD.id_full);
      assert.equal(env.asked[0].body[0], "old-desktop was last online 9 d ago.");
      assert.equal(env.asked[0].hint, undefined, "a node away for 9 days gets no 'online recently' hint");
    });

    it("says it was never seen online", async () => {
      const ghost = staleRow("ghost-key", null);
      const env = await setup({ members: membersReply({ rows: [selfRow(), peerOneRow(), ghost] }) });
      await env.pick("forget", ghost.id_full);
      assert.equal(env.asked[0].body[0], "ghost-key has never been seen online.");
    });

    it("warns when it was online within the week", async () => {
      const recent = staleRow("lost-phone", 2 * 86400);
      const env = await setup({ members: membersReply({ rows: [selfRow(), peerOneRow(), recent] }) });
      await env.pick("forget", recent.id_full);
      assert.match(env.asked[0].hint, /online recently/);
    });
  });

  describe("declined in the dialog", () => {
    it("nothing is sent", async () => {
      const env = await setup({ members: membersReply({ rows: [selfRow(), peerOneRow(), OLD] }) });
      await env.pick("forget", OLD.id_full);
      assert.deepEqual(env.writes, []);
    });
  });
});

describe("the tab changes while the removal dialog is open", () => {
  it("dialog_keeps_slot_of_opening: the request goes to the network the dialog named", async () => {
    const env = await setup({
      members: membersReply({ rows: [selfRow(), peerOneRow(), OLD] }),
      agree: true,
      reply: actionReply(membersReply({ rows: [selfRow(), peerOneRow(), { ...OLD, forget: MY_CIRCLE }] })),
      onAsk: (tab) => (tab.current = NET_A),
    });
    await env.pick("forget", OLD.id_full);
    assert.deepEqual(env.writes, [{ method: "POST", path: ABOUT_ADDRESS(NET_B, "forget"), body: { id: OLD.id_full } }]);
  });

  it("dialog_keeps_slot_of_opening: the answer lands in that network's list, not in the lead's", async () => {
    const env = await setup({
      members: membersReply({ rows: [selfRow(), peerOneRow(), OLD] }),
      agree: true,
      reply: actionReply(membersReply({ rows: [selfRow(), peerOneRow(), { ...OLD, forget: MY_CIRCLE }] })),
      onAsk: (tab) => (tab.current = NET_A),
    });
    const leadBefore = env.store.members;
    await env.pick("forget", OLD.id_full);
    assert.ok(env.store.slots[NET_B].members.members.find((m) => m.id_full === OLD.id_full).forget?.mine, "B must get the new circle");
    assert.equal(env.store.members, leadBefore, "the lead must keep its own list");
  });
});

describe("withdrawing a removal", () => {
  it("unforget_runs_without_dialog: one POST to the network of the tab", async () => {
    const circle = { ...OLD, forget: MY_CIRCLE };
    const env = await setup({
      members: membersReply({ rows: [selfRow(), peerOneRow(), circle] }),
      reply: actionReply(membersReply({ rows: [selfRow(), peerOneRow(), OLD] })),
    });
    await env.pick("unforget", OLD.id_full);
    assert.equal(env.asked.length, 0, "no dialog is opened");
    assert.deepEqual(env.writes, [{ method: "POST", path: ABOUT_ADDRESS(NET_B, "unforget"), body: { id: OLD.id_full } }]);
  });

  it("the line under the list says the member is back", async () => {
    const env = await setup({
      members: membersReply({ rows: [selfRow(), peerOneRow(), { ...OLD, forget: MY_CIRCLE }] }),
      reply: actionReply(membersReply({ rows: [selfRow(), peerOneRow(), OLD] })),
    });
    await env.pick("unforget", OLD.id_full);
    assert.deepEqual(env.actions.status.value, { text: "Removal withdrawn. old-desktop is back in the member list.", cls: "" });
  });
});

describe("a pending ban when a removal lowers the threshold", () => {
  const WARNING = "A ban is waiting for votes in this network. With old-desktop removed, fewer votes are needed, and it may pass right away.";
  const pendingBan = () => peerOneRow({ vote: foreignVote() });

  it("ban_at_risk_when_two_left_and_ban_pending: the removal that completes at once repeats the warning", async () => {
    const env = await setup({ members: membersReply({ rows: [selfRow(), pendingBan(), OLD], need: 1 }) });
    await env.pick("forget", OLD.id_full);
    assert.equal(env.asked[0].action, "Remove old-desktop");
    assert.equal(env.asked[0].body.at(-1), WARNING);
  });

  it("ban_at_risk_when_two_left_and_ban_pending: the confirming dialog repeats it too", async () => {
    const waiting = { ...OLD, forget: forgetInfo({ mine: false }) };
    const env = await setup({ members: membersReply({ rows: [selfRow(), pendingBan(), waiting] }) });
    await env.pick("forget", OLD.id_full);
    assert.equal(env.asked[0].body.at(-1), WARNING);
  });

  it("the first of two votes does not warn", async () => {
    const env = await setup({ members: membersReply({ rows: [selfRow(), pendingBan(), OLD], need: 2 }) });
    await env.pick("forget", OLD.id_full);
    assert.equal(env.asked[0].action, "Propose removal");
    assert.ok(!env.asked[0].body.includes(WARNING), "no warning on a proposal");
  });

  it("three members left: no warning", async () => {
    const env = await setup({ members: membersReply({ rows: [selfRow(), pendingBan(), staleRow("spare", 5), OLD], need: 1 }) });
    await env.pick("forget", OLD.id_full);
    assert.ok(!env.asked[0].body.includes(WARNING), "three members remain, the threshold holds");
  });

  it("no ban pending: no warning", async () => {
    const env = await setup({ members: membersReply({ rows: [selfRow(), peerOneRow(), OLD], need: 1 }) });
    await env.pick("forget", OLD.id_full);
    assert.ok(!env.asked[0].body.includes(WARNING), "nothing is waiting for votes");
  });
});

describe("the line under the list after a removal", () => {
  it("done_text_forgotten_vs_proposed: the member is gone from the list", async () => {
    const gone = { ...OLD, status: "forgotten", forget: forgetInfo({ votes: 2, mine: true, voters: ["aaaa1111", "bbbb2222"], at_unix: AT_UNIX }) };
    const env = await setup({
      members: membersReply({ rows: [selfRow(), peerOneRow(), OLD], need: 1 }),
      agree: true,
      reply: actionReply(membersReply({ rows: [selfRow(), peerOneRow(), gone] })),
    });
    await env.pick("forget", OLD.id_full);
    assert.deepEqual(env.actions.status.value, { text: "old-desktop was removed from the member list.", cls: "" });
  });

  it("done_text_forgotten_vs_proposed: the removal is only proposed", async () => {
    const env = await setup({
      members: membersReply({ rows: [selfRow(), peerOneRow(), OLD] }),
      agree: true,
      reply: actionReply(membersReply({ rows: [selfRow(), peerOneRow(), { ...OLD, forget: MY_CIRCLE }] })),
    });
    await env.pick("forget", OLD.id_full);
    const { text, cls } = env.actions.status.value;
    assert.equal(cls, "");
    assert.match(text, /^Removal proposed\. old-desktop is hidden on this computer\./);
  });

  it("a refusal shows the node's own sentence as an error", async () => {
    const env = await setup({
      members: membersReply({ rows: [selfRow(), peerOneRow(), OLD] }),
      agree: true,
      fail: Object.assign(new Error("online"), { status: 409, code: "online", body: { message: "old-desktop is online." } }),
    });
    await env.pick("forget", OLD.id_full);
    assert.deepEqual(env.actions.status.value, { text: "old-desktop is online.", cls: "field-error-text" });
  });
});

// ---------------------------------------------------------------- setup

// The Members tab of network B on a store filled from a fake node with two networks. `members` is B's list; `reply` is what
// the node answers to the POST (or `fail` rejects it); `agree` is the dialog's answer (a decline by default); `onAsk(tab)` runs while the dialog is open
// and may move the current tab (`tab.current`). `asked` lists the dialog specs, `writes` the POSTs that were sent.
async function setup({ members, reply = actionReply(membersReply()), fail = null, agree = false, onAsk = () => {} }) {
  const node = createFakeNode(twoNets());
  const viaNode = (method, path, body) => coreApi(method, path, body, { fetch: (input, init) => node.fetch(input, init), getToken: () => "t", timeoutMs: 1000, busTarget: undefined });
  const store = createStore();
  acceptOverview(store, parseOverview(await viaNode("GET", "/api/overview")));
  putSlotMembers(store, NET_B, members);
  const writes = [];
  const call = vi.fn(async (method, path, body) => {
    writes.push({ method, path, body });
    if (fail) throw fail;
    return reply;
  });
  const tab = { current: NET_B };
  const actions = useMembersActions(store, () => tab.current, { api: call, busTarget: new EventTarget() });
  const asked = [];
  actions.dialog.value = {
    ask: async (spec) => {
      asked.push(spec);
      onAsk(tab);
      return agree;
    },
  };
  return { store, actions, asked, writes, pick: (key, id) => actions.pick(key, id) };
}
