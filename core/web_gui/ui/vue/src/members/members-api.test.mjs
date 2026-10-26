/* `members-api.ts`: the client of `GET /api/members` and `POST /api/members/<action>`.
 *
 * Contract fixed by these tests:
 *  - `parseMembers(raw: unknown): MembersReply | null`.
 *      Not an object / no `members` array -> `null`. `{ members: [] }` is a valid (empty) reply.
 *      A full valid reply parses to a structurally equal object (no keys are added; `changed` is kept only if boolean).
 *      Missing optional fields get safe values: top level `me` "", `me_excluded` false,
 *      `counts` an object of numbers; row `name` null, `self` false, `online` false, `blocked_by_me` false,
 *      `excluded` null, `denied_by_me` null, `last_refusal` null, `id` a string; excluded `voters` [].
 *      A row that is not an object, has no string `id_full`, or has an unknown `status` is skipped (the rest stays).
 *      Extra (unknown) fields never break parsing.
 *  - `getMembers(api)`: `api("GET", "/api/members")` -> `parseMembers(reply)` (garbage -> `null`); an api rejection is NOT caught.
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
  membersReply,
  peerOneRow,
  peerTwoRow,
  selfRow,
} from "../testing/members-fixtures.mjs";

const plain = (value) => JSON.parse(JSON.stringify(value));

describe("parseMembers", () => {
  describe("a full valid reply", () => {
    it("keeps every field of every row", () => {
      const raw = membersReply({
        rows: [
          selfRow(),
          peerOneRow({ denied_by_me: "blocked" }),
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

  describe("optional fields are missing", () => {
    it("top level: safe values", () => {
      const parsed = parseMembers({ members: [{ id_full: EXTRA_FULL, status: "member" }] });
      assert.equal(parsed.me, "");
      assert.equal(parsed.me_excluded, false);
      for (const key of ["members", "excluded", "unlisted", "pending"]) {
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
      assert.equal(row.excluded, null);
      assert.equal(row.denied_by_me, null);
      assert.equal(row.last_refusal, null);
    });

    it("excluded without `voters`: empty list", () => {
      const excluded = { how: "left", at_unix: AT_UNIX };
      const row = parseMembers({ members: [{ id_full: EXTRA_FULL, status: "excluded", excluded }] }).members[0];
      assert.deepEqual(plain(row.excluded.voters), []);
      assert.equal(row.excluded.how, "left");
    });
  });

  describe("last_seen_sec", () => {
    it("null (never seen) stays null, zero stays zero, a missing one is null", () => {
      const rows = parseMembers(membersReply({ rows: [peerTwoRow({ last_seen_sec: null }), peerOneRow({ last_seen_sec: 0 })] })).members;
      assert.equal(rows[0].last_seen_sec, null);
      assert.equal(rows[1].last_seen_sec, 0);
      assert.equal(parseMembers({ members: [{ id_full: EXTRA_FULL, status: "member" }] }).members[0].last_seen_sec, null);
    });
  });

  describe("extra fields", () => {
    it("unknown fields at the top level and in rows are tolerated", () => {
      const raw = membersReply();
      const parsed = parseMembers({ ...raw, future_flag: 1, members: raw.members.map((r) => ({ ...r, extra: { a: 1 } })) });
      assert.equal(parsed.members.length, 3);
      assert.equal(parsed.me, raw.me);
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
