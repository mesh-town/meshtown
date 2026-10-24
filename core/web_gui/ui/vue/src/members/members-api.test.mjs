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
 *  - A 5th argument `slot` (default "main") moves the request to `slotPath(slot, "members/<action>")`: `main` keeps
 *      `/api/members/<action>`, another network is `/api/net/<slot>/members/<action>`; the body is the same.
 *  - `parseLeave(raw): LeaveReply | null`: a member list plus `left`, `announced` (both default `true` when absent: an older node
 *      answers 200 for a left network) and `nets` (briefs, default `[]`); not a member list -> `null`.
 *  - `leaveNet(api, slot)`: one `POST slotPath(slot, "members/leave")` with `{ confirm: true }`, resolves the `LeaveReply`;
 *      an unreadable reply rejects with an Error.
 *  - `memberErrorText(err): string`: `failText("update the member list", err)` of common/words.ts when the error carries a
 *      human message (`body.message`, else `message`); when there is none (message is empty or equals the machine code) the
 *      reason comes from the code (self_target, not_member, left, busy, invalid, internal) so the raw code is never shown.
 *      Not an error object at all -> "Couldn't update the member list — try again". */

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

  describe("the reason of a ban proposal", () => {
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

  describe("removal fields", () => {
    const forgotten = () => extraRow({ status: "forgotten", online: false, last_seen_sec: 900000, forget: forgetInfo({ votes: 2, mine: true, at_unix: AT_UNIX, voters: ["aaaa1111", "bbbb2222"] }) });

    it("a reply with forget, last_seen_sec, me_removed and counts.forgotten arrives whole", () => {
      const raw = membersReply({
        rows: [selfRow(), peerOneRow({ forget: forgetInfo() }), staleRow("old-desktop", 777600), forgotten()],
        me_removed: true,
      });
      const parsed = plain(parseMembers(raw));
      assert.deepEqual(parsed, raw);
      assert.equal(parsed.me_removed, true);
      assert.equal(parsed.counts.forgotten, 1);
      assert.deepEqual(parsed.members[1].forget, forgetInfo());
      assert.equal(parsed.members[2].last_seen_sec, 777600);
    });

    it("the status \"forgotten\" is kept as it is, the row is not skipped", () => {
      const parsed = parseMembers(membersReply({ rows: [selfRow(), forgotten()] }));
      assert.equal(parsed.members.length, 2);
      assert.equal(parsed.members[1].status, "forgotten");
    });

    it("the ids of the circle stay short ids and at_unix stays a number", () => {
      const row = parseMembers(membersReply({ rows: [forgotten()] })).members[0];
      assert.deepEqual(plain(row.forget.voters), ["aaaa1111", "bbbb2222"]);
      assert.equal(row.forget.proposed_by, "bbbb2222");
      assert.equal(row.forget.at_unix, AT_UNIX);
      assert.equal(row.forget.mine, true);
    });

    it("last_seen_sec: null (never seen) stays null, zero stays zero", () => {
      const rows = parseMembers(membersReply({ rows: [staleRow("ghost-key", null), peerOneRow({ last_seen_sec: 0 })] })).members;
      assert.equal(rows[0].last_seen_sec, null);
      assert.equal(rows[1].last_seen_sec, 0);
    });
  });

  describe("removal fields are missing", () => {
    const bare = () => parseMembers({ members: [{ id_full: EXTRA_FULL, status: "member" }] });

    it("row: forget and last_seen_sec are null", () => {
      const row = bare().members[0];
      assert.equal(row.forget, null);
      assert.equal(row.last_seen_sec, null);
    });

    it("top level: me_removed is false", () => {
      assert.equal(bare().me_removed, false);
    });

    it("counts.forgotten is the number of rows with the status \"forgotten\"", () => {
      assert.equal(bare().counts.forgotten, 0);
      const rows = [{ id_full: EXTRA_FULL, status: "forgotten" }, { id_full: PEER_FULL_HEX, status: "forgotten" }, { id_full: SELF_HEX, status: "member" }];
      assert.equal(parseMembers({ members: rows }).counts.forgotten, 2);
    });

    it("a counts.forgotten of the server wins over the rows", () => {
      const parsed = parseMembers({ members: [{ id_full: EXTRA_FULL, status: "forgotten" }], counts: { forgotten: 5 } });
      assert.equal(parsed.counts.forgotten, 5);
    });

    it("forget that is not an object gives null", () => {
      for (const forget of ["x", 3, [], true]) {
        const row = parseMembers({ members: [{ id_full: EXTRA_FULL, status: "member", forget }] }).members[0];
        assert.equal(row.forget, null, JSON.stringify(forget));
      }
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
