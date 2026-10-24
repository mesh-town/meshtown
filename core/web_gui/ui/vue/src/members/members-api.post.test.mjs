/* `members-api.ts`: `getMembers` and `postMemberAction`. The contract is in `members-api.test.mjs`. */

import { describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { getMembers, parseMembers, postMemberAction } from "./members-api.ts";
import { NET_A } from "../testing/overview-fixtures.mjs";
import {
  AT_UNIX,
  EXTRA_FULL,
  excludedInfo,
  extraRow,
  forgetInfo,
  membersReply,
  peerOneRow,
  peerTwoRow,
  selfRow,
  staleRow,
  voteInfo,
} from "../testing/members-fixtures.mjs";

const plain = (value) => JSON.parse(JSON.stringify(value));
const PEER_FULL_HEX = "B".repeat(64);
const SELF_HEX = "A".repeat(64);


describe("getMembers", () => {
  it("asks GET /api/members and returns the parsed reply", async () => {
    const { api, call } = setup({ reply: membersReply() });
    const result = await getMembers(api);
    assert.deepEqual(call(0), { method: "GET", path: "/api/members", body: undefined });
    assert.deepEqual(plain(result), membersReply());
  });

  it("a reply that is not a members list gives null", async () => {
    const { api } = setup({ reply: { hello: "world" } });
    assert.equal(await getMembers(api), null);
  });

  it("an empty body (204) gives null", async () => {
    const { api } = setup({ reply: null });
    assert.equal(await getMembers(api), null);
  });

  it("an api error is not swallowed", async () => {
    const { api } = setup({ fail: new ApiError(409, "busy") });
    await assert.rejects(getMembers(api), (err) => err.status === 409 && err.code === "busy");
  });
});

describe("postMemberAction", () => {
  describe("the request", () => {
    it.each(["exclude", "unexclude", "block", "unblock"])("%s: POST /api/members/%s with the id", async (action) => {
      const { api, call } = setup({ reply: { ...membersReply(), changed: true } });
      await postMemberAction(api, action, EXTRA_FULL);
      assert.deepEqual(call(0), { method: "POST", path: "/api/members/" + action, body: { id: EXTRA_FULL } });
    });

    it("leave: the body is { confirm: true }, an id is not sent", async () => {
      const { api, call } = setup({ reply: { ...membersReply(), changed: true } });
      await postMemberAction(api, "leave");
      assert.deepEqual(call(0), { method: "POST", path: "/api/members/leave", body: { confirm: true } });
    });

    it("leave ignores a stray id", async () => {
      const { api, call } = setup({ reply: { ...membersReply(), changed: true } });
      await postMemberAction(api, "leave", EXTRA_FULL);
      assert.deepEqual(call(0).body, { confirm: true });
    });
  });

  describe("removal of a member", () => {
    it.each(["forget", "unforget"])("%s: POST /api/members/%s with the id", async (action) => {
      const { api, call } = setup({ reply: { ...membersReply(), changed: true } });
      await postMemberAction(api, action, EXTRA_FULL);
      assert.deepEqual(call(0), { method: "POST", path: "/api/members/" + action, body: { id: EXTRA_FULL } });
    });

    it("forget goes to the path of the slot, with { id }", async () => {
      const { api, call } = setup({ reply: { ...membersReply(), changed: true } });
      await postMemberAction(api, "forget", EXTRA_FULL, undefined, "n2");
      assert.deepEqual(call(0), { method: "POST", path: "/api/net/n2/members/forget", body: { id: EXTRA_FULL } });
    });

    it("unforget goes to the path of the slot, with { id }", async () => {
      const { api, call } = setup({ reply: { ...membersReply(), changed: true } });
      await postMemberAction(api, "unforget", EXTRA_FULL, undefined, "n2");
      assert.deepEqual(call(0), { method: "POST", path: "/api/net/n2/members/unforget", body: { id: EXTRA_FULL } });
    });

    it("a reason is never sent with forget", async () => {
      const { api, call } = setup({ reply: { ...membersReply(), changed: true } });
      await postMemberAction(api, "forget", EXTRA_FULL, "lost laptop");
      assert.deepEqual(call(0).body, { id: EXTRA_FULL });
    });
  });

  describe("the reason of a ban", () => {
    const bodyOf = async (action, id, reason) => {
      const { api, call } = setup({ reply: { ...membersReply(), changed: true } });
      await postMemberAction(api, action, id, reason);
      return call(0).body;
    };

    it("exclude with a reason: the body is { id, reason }", async () => {
      assert.deepEqual(await bodyOf("exclude", EXTRA_FULL, "lost laptop"), { id: EXTRA_FULL, reason: "lost laptop" });
    });

    it("exclude without a reason: the body is { id } and has no `reason` key", async () => {
      const body = await bodyOf("exclude", EXTRA_FULL);
      assert.deepEqual(body, { id: EXTRA_FULL });
      assert.equal("reason" in body, false, "a `reason` key was sent: " + JSON.stringify(body));
    });

    it.each([[""], ["   "], ["\n\t"], [undefined]])("exclude with the reason %j: no `reason` key", async (reason) => {
      const body = await bodyOf("exclude", EXTRA_FULL, reason);
      assert.deepEqual(body, { id: EXTRA_FULL });
      assert.equal("reason" in body, false, "a `reason` key was sent: " + JSON.stringify(body));
    });

    // `cleanReason` is applied to the reason before it is sent.
    it("exclude cleans the reason: the ends are trimmed and a newline becomes a space", async () => {
      assert.deepEqual(await bodyOf("exclude", EXTRA_FULL, "  lost\nlaptop "), { id: EXTRA_FULL, reason: "lost laptop" });
    });

    it.each(["unexclude", "block", "unblock"])("%s never sends a reason", async (action) => {
      assert.deepEqual(await bodyOf(action, EXTRA_FULL, "lost laptop"), { id: EXTRA_FULL });
    });

    it("leave sends only the confirmation, whatever the reason", async () => {
      assert.deepEqual(await bodyOf("leave", undefined, "lost laptop"), { confirm: true });
    });
  });

  describe("the result", () => {
    it("the parsed reply without `changed`, and `changed` next to it", async () => {
      const reply = membersReply({ rows: [selfRow(), peerOneRow({ vote: voteInfo() })] });
      const { api } = setup({ reply: { ...reply, changed: true } });
      const result = await postMemberAction(api, "exclude", peerOneRow().id_full);
      assert.equal(result.changed, true);
      assert.deepEqual(plain(result.reply), reply);
      assert.equal(result.reply.changed, undefined, "the reply part must not carry `changed`");
    });

    it("a repeated action (changed: false) is passed on", async () => {
      const { api } = setup({ reply: { ...membersReply(), changed: false } });
      const result = await postMemberAction(api, "block", EXTRA_FULL);
      assert.equal(result.changed, false);
    });
  });

  describe("failures", () => {
    it("an api error propagates as is", async () => {
      const { api } = setup({ fail: new ApiError(409, "self_target") });
      await assert.rejects(postMemberAction(api, "exclude", EXTRA_FULL), (err) => err.status === 409 && err.code === "self_target");
    });

    // A 200 with a body we cannot read is an error.
    it("a reply that is not a members list rejects", async () => {
      const { api } = setup({ reply: { ok: true } });
      await assert.rejects(postMemberAction(api, "block", EXTRA_FULL), Error);
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
