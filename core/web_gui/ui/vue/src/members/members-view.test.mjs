/* `members-view.ts`, part 1: lookups, the grouping of rows, dates and the "excluded" lines.
 * (Row decoration and the `⋯` menu are in `members-view-rows.test.mjs`.) All functions are pure, named exports of `./members-view.ts`.
 *
 * Contract fixed by these tests:
 *  - `MemberRow` / `MembersReply` are the objects of `GET /api/members` (see `testing/members-fixtures.mjs`); `me` is the
 *    `id_full` of this node. A "peer" is an item of `state.peers` (`PeerInfo`).
 *  - `memberById(members, idFull): MemberRow | null` — `null` when `members` is null or the id is unknown.
 *  - `meId(state): string` — `state.node.id_full`; `""` when there is no state or no node.
 *  - `isMeExcluded(state, members): boolean` — `state.members.me_excluded || members.me_excluded`, both may be missing/null.
 *  - `excludedVariant(members, me): "left" | "banned"` — "left" only when my own row has `excluded.how === "left"`.
 *  - `displayNames(members): Record<id_full, string>` — name, or the short `id` for a nameless/blank-named member; `{}` for null.
 *  - `partitionRows(peers, members): { active, pending, unlisted, excluded }`:
 *      `active` holds the peers with status "member" (the same objects as in `peers`, order kept) and, appended in the order of
 *      `members`, a minimal row `{ id, id_full, name, online, self }` copied from the member row for a member absent from `peers`;
 *      `unlisted` and `excluded` hold `MemberRow`s of `members` (order of `members`). A peer unknown to `members` stays in `active`.
 *      `members == null`: `{ active: peers, pending: [], unlisted: [], excluded: [] }`. An offline member stays in `active`.
 *  - `shortDate(unix, timeZone?): string` — "19 Oct" (day without a leading zero, English short month); a non-number gives "".
 *  - `excludedRowLine(m, names, timeZone?): string`; `names` is a `displayNames` object; an unknown id falls back to its first 8 chars. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import {
  displayNames,
  excludedVariant,
  isMeExcluded,
  meId,
  memberById,
  partitionRows,
  excludedRowLine,
  shortDate,
} from "./members-view.ts";
import { PEER2_FULL, PEER_FULL, SELF_FULL } from "../testing/section.mjs";
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

const ME = SELF_FULL;
const peer = (idFull, over = {}) => ({ id: idFull.slice(0, 8).toLowerCase(), id_full: idFull, name: null, online: true, ...over });
const ids = (rows) => rows.map((r) => r.id_full);

describe("memberById", () => {
  it("finds the row by id_full", () => {
    const reply = membersReply();
    assert.equal(memberById(reply, PEER_FULL), reply.members[1]);
  });

  it("an unknown id gives null", () => {
    assert.equal(memberById(membersReply(), EXTRA_FULL), null);
  });

  it("no members at all gives null", () => {
    assert.equal(memberById(null, PEER_FULL), null);
  });
});

describe("meId", () => {
  it("is the full id of this node", () => {
    assert.equal(meId({ node: { id: "aaaa1111", id_full: SELF_FULL } }), SELF_FULL);
  });

  it.each([[null], [undefined], [{}], [{ node: {} }]])("%j: an empty string", (state) => {
    assert.equal(meId(state), "");
  });
});

describe("isMeExcluded", () => {
  it.each([
    ["the summary of the snapshot says so", { members: { me_excluded: true } }, membersReply(), true],
    ["the members reply says so", { members: { me_excluded: false } }, membersReply({ me_excluded: true }), true],
    ["both say so", { members: { me_excluded: true } }, membersReply({ me_excluded: true }), true],
    ["neither says so", { members: { me_excluded: false } }, membersReply(), false],
    ["the members reply is absent, the summary says so", { members: { me_excluded: true } }, null, true],
    ["the snapshot is absent, the reply says so", null, membersReply({ me_excluded: true }), true],
    ["the summary has no such field (an old node)", { members: {} }, null, false],
    ["nothing is known", null, null, false],
  ])("%s", (_name, state, members, expected) => {
    assert.equal(isMeExcluded(state, members), expected);
  });
});

describe("excludedVariant", () => {
  it("my own row says I left: left", () => {
    const reply = membersReply({ rows: [selfRow({ status: "excluded", excluded: excludedInfo({ how: "left", voters: [ME] }) })] });
    assert.equal(excludedVariant(reply, ME), "left");
  });

  it("my own row says I was banned by an admin: banned", () => {
    const reply = membersReply({ rows: [selfRow({ status: "excluded", excluded: excludedInfo({ how: "removed" }) })] });
    assert.equal(excludedVariant(reply, ME), "banned");
  });

  it("somebody else left, not me: banned", () => {
    const reply = membersReply({ rows: [selfRow(), extraRow({ status: "excluded", excluded: excludedInfo({ how: "left" }) })] });
    assert.equal(excludedVariant(reply, ME), "banned");
  });

  it("no members or no row of mine: banned", () => {
    assert.equal(excludedVariant(null, ME), "banned");
    assert.equal(excludedVariant(membersReply({ rows: [peerOneRow()] }), ME), "banned");
  });
});

describe("displayNames", () => {
  it("a name when there is one, else the short id", () => {
    const names = displayNames(membersReply({ rows: [selfRow(), peerOneRow(), peerTwoRow({ name: null }), extraRow({ name: "  " })] }));
    assert.deepEqual(names, { [SELF_FULL]: "finch-unwary", [PEER_FULL]: "peer-one", [PEER2_FULL]: "cccc3333", [EXTRA_FULL]: "dddd4444" });
  });

  it("no members: an empty object", () => {
    assert.deepEqual(displayNames(null), {});
  });
});

describe("shortDate", () => {
  it("day and short month in UTC", () => {
    assert.equal(shortDate(AT_UNIX, "UTC"), "19 Oct");
  });

  it("the day has no leading zero", () => {
    assert.equal(shortDate(Date.UTC(2026, 10, 5, 12) / 1000, "UTC"), "5 Nov");
  });

  it("the time zone moves the day", () => {
    const late = Date.UTC(2026, 9, 19, 23, 30) / 1000;
    assert.equal(shortDate(late, "UTC"), "19 Oct");
    assert.equal(shortDate(late, "Asia/Tokyo"), "20 Oct");
    assert.equal(shortDate(AT_UNIX, "Pacific/Pago_Pago"), "18 Oct");
  });

  it.each([[undefined], [null], [NaN], ["1792400000"]])("%j is not a timestamp: an empty string", (value) => {
    assert.equal(shortDate(value, "UTC"), "");
  });
});

describe("excludedRowLine", () => {
  const names = { [SELF_FULL]: "finch-unwary", [PEER_FULL]: "peer-one" };

  describe("removed by an admin", () => {
    it("lists the date and both voters", () => {
      const row = extraRow({ status: "excluded", excluded: excludedInfo({ how: "removed", voters: [SELF_FULL, PEER_FULL] }) });
      assert.equal(excludedRowLine(row, names, "UTC"), "old-laptop · removed on 19 Oct · by finch-unwary and peer-one");
    });

    it("a nameless member is shown by its short id", () => {
      const row = peerTwoRow({ status: "excluded", excluded: excludedInfo({ how: "removed", voters: [SELF_FULL, PEER_FULL] }) });
      assert.equal(excludedRowLine(row, names, "UTC"), "cccc3333 · removed on 19 Oct · by finch-unwary and peer-one");
    });

    it("a single voter: no \"and\"", () => {
      const row = extraRow({ status: "excluded", excluded: excludedInfo({ how: "removed", voters: [PEER_FULL] }) });
      assert.equal(excludedRowLine(row, names, "UTC"), "old-laptop · removed on 19 Oct · by peer-one");
    });

    // NOTE: a voter that is in neither the peers nor the list — the first 8 characters of the id are the likely fallback.
    it("a voter nobody knows is shown by the start of its id", () => {
      const row = extraRow({ status: "excluded", excluded: excludedInfo({ how: "removed", voters: [SELF_FULL, "e".repeat(64)] }) });
      assert.equal(excludedRowLine(row, names, "UTC"), "old-laptop · removed on 19 Oct · by finch-unwary and eeeeeeee");
    });
  });

  describe("no admin is known", () => {
    it("drops the \"by\" part", () => {
      const row = extraRow({ status: "excluded", excluded: excludedInfo({ how: "removed", voters: [] }) });
      assert.equal(excludedRowLine(row, names, "UTC"), "old-laptop · removed on 19 Oct");
    });
  });

  describe("left on their own", () => {
    it("says that the member left the network", () => {
      const row = extraRow({ status: "excluded", excluded: excludedInfo({ how: "left", voters: [EXTRA_FULL] }) });
      assert.equal(excludedRowLine(row, names, "UTC"), "old-laptop · left the network on 19 Oct");
    });
  });
});
