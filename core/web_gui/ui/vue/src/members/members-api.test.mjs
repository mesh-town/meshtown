/* `members-api.ts`: the client of `GET /api/members` and `POST /api/members/<action>`.
 *
 * Contract fixed by these tests:
 *  - `parseMembers(raw: unknown): MembersReply | null`.
 *      Not an object / no `members` array -> `null`. `{ members: [] }` is a valid (empty) reply.
 *      A full valid reply parses to a structurally equal object (no keys are added; `changed` is kept only if boolean).
 *      Missing optional fields get safe values: top level `me` "", `me_excluded` false, `need` an integer >= 1 (2),
 *      `counts` an object of four numbers; row `name` null, `self` false, `online` false, `blocked_by_me` false,
 *      `vote` null, `excluded` null, `denied_by_me` null, `last_refusal` null, `id` a string; vote `voters` [], `mine` false;
 *      excluded `voters` []; vote `reason` null.
 *      A row that is not an object, has no string `id_full`, or has an unknown `status` is skipped (the rest stays).
 *      Extra (unknown) fields never break parsing.
 *  - `getMembers(api)`: `api("GET", "/api/members")` -> `parseMembers(reply)` (garbage -> `null`); an api rejection is NOT caught.
 *  - `vote.reason` (the optional text of the proposer, up to 80 characters): a string with something besides spaces is kept
 *      (the page shows it; trimming is allowed), anything else (missing, null, empty or blank, not a string) is `null`.
 *  - `postMemberAction(api, action, id?, reason?)`: `api("POST", "/api/members/" + action, body)`, body `{ id }`, for "leave"
 *      `{ confirm: true }` (id ignored). Only for "exclude" a non-blank `reason` joins the body as `{ id, reason }` (cleaned by
 *      `cleanReason`); a blank reason leaves the body `{ id }` with no `reason` key at all; other actions never send a reason.
 *      Resolves `{ reply: MembersReply, changed: boolean }`; `reply` has NO `changed` key. Rejections propagate;
 *      a reply that cannot be parsed makes it reject with an Error.
 *  - `memberErrorText(err): string`: `failText("update the member list", err)` of common/words.ts when the error carries a
 *      human message (`body.message`, else `message`); when there is none (message is empty or equals the machine code) the
 *      reason comes from the code (self_target, not_member, left, busy, invalid, internal) so the raw code is never shown.
 *      Not an error object at all -> "Couldn't update the member list — try again". */

import { describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { getMembers, memberErrorText, parseMembers, postMemberAction } from "./members-api.ts";
import {
  AT_UNIX,
  EXTRA_FULL,
  excludedInfo,
  extraRow,
  membersReply,
  peerOneRow,
  peerTwoRow,
  selfRow,
  voteInfo,
} from "../testing/members-fixtures.mjs";

const plain = (value) => JSON.parse(JSON.stringify(value));
const ACTION = "update the member list";

describe("parseMembers", () => {
  describe("a full valid reply", () => {
    it("keeps every field of every row", () => {
      const raw = membersReply({
        rows: [
          selfRow(),
          peerOneRow({ vote: voteInfo({ reason: null }), denied_by_me: "voted_by_me" }),
          peerTwoRow({ blocked_by_me: true, denied_by_me: "blocked" }),
          extraRow({
            status: "excluded",
            excluded: excludedInfo(),
            denied_by_me: "excluded",
            last_refusal: { side: "me", reason: "excluded", at_unix: AT_UNIX },
          }),
        ],
      });
      assert.deepEqual(plain(parseMembers(raw)), raw);
    });

    it("an empty member list is a valid reply, not null", () => {
      const parsed = parseMembers(membersReply({ rows: [] }));
      assert.deepEqual(parsed.members, []);
    });

    it("`changed` of an action reply is kept", () => {
      assert.equal(parseMembers({ ...membersReply(), changed: true }).changed, true);
      assert.equal(parseMembers({ ...membersReply(), changed: false }).changed, false);
    });

    it("no `changed` in the reply — no `changed` in the result", () => {
      assert.equal(parseMembers(membersReply()).changed, undefined);
    });
  });

  describe("the reason of a removal proposal", () => {
    const withVote = (vote) => parseMembers({ members: [{ id_full: EXTRA_FULL, status: "member", vote }] }).members[0].vote;
    const baseVote = { votes: 1, need: 2, proposed_by: EXTRA_FULL, voters: [EXTRA_FULL], mine: false };

    it("a text is kept with the rest of the vote", () => {
      const raw = membersReply({ rows: [selfRow(), peerOneRow({ vote: voteInfo({ reason: "lost laptop" }) })] });
      assert.deepEqual(plain(parseMembers(raw)), raw);
      assert.equal(parseMembers(raw).members[1].vote.reason, "lost laptop");
    });

    it("an eighty-character text is kept whole", () => {
      const text = "x".repeat(80);
      assert.equal(withVote({ ...baseVote, reason: text }).reason, text);
    });

    it("`null` stays null", () => {
      assert.equal(withVote({ ...baseVote, reason: null }).reason, null);
    });

    it("no `reason` key (an older node) gives null, not undefined", () => {
      assert.equal(withVote(baseVote).reason, null);
    });

    it.each([
      ["an empty string", ""],
      ["only spaces", "   "],
      ["only a newline and a tab", "\n\t"],
      ["a number", 7],
      ["an object", { text: "lost laptop" }],
      ["a list", ["lost laptop"]],
      ["a boolean", true],
    ])("%s gives null", (_name, reason) => {
      assert.equal(withVote({ ...baseVote, reason }).reason, null);
    });

    it("a text with spaces around it still carries the words", () => {
      const parsed = withVote({ ...baseVote, reason: "  lost laptop " }).reason;
      assert.equal(typeof parsed, "string");
      assert.equal(parsed.trim(), "lost laptop");
    });
  });

  describe("optional fields are missing", () => {
    it("top level: safe values", () => {
      const parsed = parseMembers({ members: [{ id_full: EXTRA_FULL, status: "member" }] });
      assert.equal(parsed.me, "");
      assert.equal(parsed.me_excluded, false);
      assert.ok(Number.isInteger(parsed.need) && parsed.need >= 1, "need must be a positive integer, got " + parsed.need);
      for (const key of ["members", "voting", "excluded", "unlisted"]) {
        assert.equal(typeof parsed.counts[key], "number", "counts." + key + " must be a number");
      }
    });

    it("row: safe values", () => {
      const row = parseMembers({ members: [{ id_full: EXTRA_FULL, status: "member" }] }).members[0];
      assert.equal(row.id_full, EXTRA_FULL);
      assert.equal(row.status, "member");
      assert.equal(typeof row.id, "string");
      assert.equal(row.name, null);
      assert.equal(row.self, false);
      assert.equal(row.online, false);
      assert.equal(row.blocked_by_me, false);
      assert.equal(row.vote, null);
      assert.equal(row.excluded, null);
      assert.equal(row.denied_by_me, null);
      assert.equal(row.last_refusal, null);
    });

    it("vote without `voters` and `mine`: empty list and false", () => {
      const vote = { votes: 1, need: 2, proposed_by: EXTRA_FULL };
      const row = parseMembers({ members: [{ id_full: EXTRA_FULL, status: "member", vote }] }).members[0];
      assert.deepEqual(plain(row.vote.voters), []);
      assert.equal(row.vote.mine, false);
      assert.equal(row.vote.proposed_by, EXTRA_FULL);
    });

    it("excluded without `voters`: empty list", () => {
      const excluded = { how: "left", at_unix: AT_UNIX };
      const row = parseMembers({ members: [{ id_full: EXTRA_FULL, status: "excluded", excluded }] }).members[0];
      assert.deepEqual(plain(row.excluded.voters), []);
      assert.equal(row.excluded.how, "left");
    });
  });

  describe("extra fields", () => {
    it("unknown fields at the top level and in rows are tolerated", () => {
      const raw = membersReply();
      const parsed = parseMembers({ ...raw, future_flag: 1, members: raw.members.map((r) => ({ ...r, extra: { a: 1 } })) });
      assert.equal(parsed.members.length, 3);
      assert.equal(parsed.need, raw.need);
      assert.equal(parsed.members[1].id_full, raw.members[1].id_full);
    });
  });

  describe("broken rows", () => {
    // A malformed row is skipped, the rest is kept.
    it("rows that are not objects, have no id_full or an unknown status are skipped", () => {
      const good = peerOneRow();
      const parsed = parseMembers({
        members: [null, "x", 7, { status: "member" }, { id_full: EXTRA_FULL, status: "banned" }, good],
      });
      assert.deepEqual(
        parsed.members.map((r) => r.id_full),
        [good.id_full],
      );
    });
  });

  describe("garbage", () => {
    it.each([
      ["null", null],
      ["undefined", undefined],
      ["a string", "members"],
      ["a number", 42],
      ["an array", []],
      ["an object without members", {}],
      ["members is not an array", { members: "x" }],
      ["members is an object", { members: {} }],
    ])("%s -> null", (_name, raw) => {
      assert.equal(parseMembers(raw), null);
    });
  });
});

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

  describe("the reason of a removal", () => {
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

describe("memberErrorText", () => {
  describe("the node sent a message", () => {
    it.each([
      [409, "self_target", "You can't remove your own computer"],
      [409, "not_member", "That computer isn't in the member list"],
      [409, "left", "This computer has left the network"],
      [409, "busy", "The node is starting"],
      [422, "invalid", "The member id is not valid"],
      [500, "internal", "Couldn't write the member list"],
    ])("%i %s: Couldn't …: the message with a lowercase first letter", (status, code, message) => {
      const err = new ApiError(status, code, message, { error: code, message });
      const lowered = message[0].toLowerCase() + message.slice(1);
      assert.equal(memberErrorText(err), "Couldn't " + ACTION + ": " + lowered);
    });

    it("the program name at the start keeps its capital letter", () => {
      const err = new ApiError(409, "busy", "busy", { error: "busy", message: "Mesh Town is stopping" });
      assert.equal(memberErrorText(err), "Couldn't " + ACTION + ": Mesh Town is stopping");
    });
  });

  describe("no message, only a code", () => {
    it.each([
      [409, "self_target"],
      [409, "not_member"],
      [409, "left"],
      [409, "busy"],
      [422, "invalid"],
      [500, "internal"],
    ])("%i %s: a human reason, never the raw code", (status, code) => {
      const text = memberErrorText(new ApiError(status, code));
      assert.ok(text.startsWith("Couldn't " + ACTION + ": "), text);
      assert.ok(!text.endsWith(": " + code), "the raw code is the whole reason: " + text);
      assert.ok(!text.includes("_"), "a machine code leaked: " + text);
      assert.ok(text.length > ("Couldn't " + ACTION + ": ").length + 5, "the reason is empty: " + text);
    });

    it("different codes give different reasons", () => {
      const codes = ["self_target", "not_member", "left", "busy", "invalid", "internal"];
      const texts = new Set(codes.map((code) => memberErrorText(new ApiError(409, code))));
      assert.equal(texts.size, codes.length);
    });
  });

  describe("the page could not reach the node", () => {
    it("network: the message of the api layer", () => {
      const err = new ApiError(0, "network", "Mesh Town is not responding");
      assert.equal(memberErrorText(err), "Couldn't " + ACTION + ": Mesh Town is not responding");
    });

    it("timeout: the message of the api layer", () => {
      const err = new ApiError(0, "timeout", "Mesh Town didn't respond in time");
      assert.equal(memberErrorText(err), "Couldn't " + ACTION + ": Mesh Town didn't respond in time");
    });
  });

  describe("not an api error", () => {
    it("a plain Error: its message", () => {
      assert.equal(memberErrorText(new Error("Boom")), "Couldn't " + ACTION + ": boom");
    });

    it.each([[null], [undefined], [{}]])("%j: the generic phrase with a hint", (err) => {
      assert.equal(memberErrorText(err), "Couldn't " + ACTION + " — try again");
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
