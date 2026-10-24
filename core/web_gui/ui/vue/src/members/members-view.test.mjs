/* `members-view.ts`, part 1: lookups, the "can I confirm" rules, the grouping of rows, dates and the "excluded" lines.
 * (Row decoration and the `⋯` menu are in `members-view-rows.test.mjs`.) All functions are pure, named exports of `./members-view.ts`.
 *
 * Contract fixed by these tests:
 *  - `MemberRow` / `MembersReply` are the objects of `GET /api/members` (see `testing/members-fixtures.mjs`); `me` is the
 *    `id_full` of this node. A "peer" is an item of `state.peers` (`PeerInfo`).
 *  - `memberById(members, idFull): MemberRow | null` — `null` when `members` is null or the id is unknown.
 *  - `meId(state): string` — `state.node.id_full`; `""` when there is no state or no node.
 *  - `canConfirm(m, me): boolean` — strictly `true`/`false` (never a vote object); `false` for a null/undefined `m`.
 *  - `confirmable(members, me): MemberRow[]` — rows with `canConfirm`, in the order of `members.members`; `[]` for null members.
 *  - `isMeExcluded(state, members): boolean` — `state.members.me_excluded || members.me_excluded`, both may be missing/null.
 *  - `excludedVariant(members, me): "left" | "banned"` — "left" only when my own row has `excluded.how === "left"`.
 *  - `displayNames(members): Record<id_full, string>` — name, or the short `id` for a nameless/blank-named member; `{}` for null.
 *      (It builds the `names` argument of `excludedRowLine` and `rowDecor`.)
 *  - `partitionRows(peers, members, me): { active, pending, unlisted, excluded }`:
 *      `active` holds the peers with status "member" (the same objects as in `peers`, order kept) and, appended in the order of
 *      `members`, a minimal row `{ id, id_full, name, online, self }` copied from the member row for a member absent from `peers`;
 *      `unlisted` and `excluded` hold `MemberRow`s of `members` (status "unlisted" / "excluded", order of `members`) — they are
 *      drawn from the member data (`UnlistedList.vue` / `ExcludedList.vue`), whether or not the node is among the peers.
 *      A peer unknown to `members` stays in `active`. `members == null`: `{ active: peers, unlisted: [], excluded: [] }`.
 *  - `shortDate(unix, timeZone?): string` — "19 Oct" (day without a leading zero, English short month); a non-number gives "".
 *  - `excludedRowLine(m, names, timeZone?): string`; `names` is a `displayNames` object; an unknown id falls back to its first 8 chars. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import {
  canConfirm,
  confirmable,
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
  foreignVote,
  membersReply,
  peerOneRow,
  peerTwoRow,
  selfRow,
  voteInfo,
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

describe("canConfirm", () => {
  describe("a vote of someone else on a member", () => {
    it("I can confirm it", () => {
      assert.equal(canConfirm(peerOneRow({ vote: voteInfo({ proposed_by: PEER2_FULL, mine: false }) }), ME), true);
    });
  });

  describe("I can not confirm", () => {
    it.each([
      ["there is no vote", peerOneRow()],
      ["the vote is already mine", peerOneRow({ vote: voteInfo({ proposed_by: PEER2_FULL, mine: true }) })],
      ["I proposed it", peerOneRow({ vote: voteInfo({ proposed_by: ME, mine: false }) })],
      ["I am the target", selfRow({ vote: foreignVote() })],
      ["the row is excluded", peerOneRow({ status: "excluded", vote: foreignVote() })],
      ["the row is not in the list", peerOneRow({ status: "unlisted", vote: foreignVote() })],
    ])("%s", (_name, row) => {
      assert.equal(canConfirm(row, ME), false);
    });

    it("no row", () => {
      assert.equal(canConfirm(null, ME), false);
      assert.equal(canConfirm(undefined, ME), false);
    });
  });
});

describe("confirmable", () => {
  it("only the rows I can confirm, in the order of the list", () => {
    const reply = membersReply({
      rows: [selfRow({ vote: foreignVote() }), peerOneRow({ vote: foreignVote() }), peerTwoRow({ vote: voteInfo() }), extraRow({ vote: foreignVote() })],
    });
    assert.deepEqual(ids(confirmable(reply, ME)), [PEER_FULL, EXTRA_FULL]);
  });

  it("no votes: empty", () => {
    assert.deepEqual(confirmable(membersReply(), ME), []);
  });

  it("no members: empty", () => {
    assert.deepEqual(confirmable(null, ME), []);
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

  it("my own row says I was banned by votes: banned", () => {
    const reply = membersReply({ rows: [selfRow({ status: "excluded", excluded: excludedInfo({ how: "votes" }) })] });
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

describe("partitionRows", () => {
  describe("members are not known (an old node or a failed request)", () => {
    it("every peer stays in the active list as it is", () => {
      const peers = [peer(PEER_FULL), peer(PEER2_FULL)];
      const parts = partitionRows(peers, null, ME);
      assert.deepEqual(parts.active, peers);
      assert.equal(parts.active[0], peers[0], "the very same peer objects");
      assert.deepEqual(parts.unlisted, []);
      assert.deepEqual(parts.excluded, []);
    });

    it("no peers either: three empty lists", () => {
      assert.deepEqual(partitionRows(null, null, ME), { active: [], pending: [], unlisted: [], excluded: [] });
    });
  });

  describe("everybody is a member", () => {
    it("the active list is the peers in their order", () => {
      const peers = [peer(PEER2_FULL), peer(SELF_FULL, { self: true }), peer(PEER_FULL)];
      const reply = membersReply({ rows: [selfRow(), peerOneRow(), peerTwoRow()] });
      const parts = partitionRows(peers, reply, ME);
      assert.deepEqual(ids(parts.active), [PEER2_FULL, SELF_FULL, PEER_FULL]);
      assert.equal(parts.unlisted.length + parts.excluded.length, 0);
    });

    it("a pending vote does not move a row anywhere", () => {
      const peers = [peer(SELF_FULL, { self: true }), peer(PEER_FULL)];
      const reply = membersReply({ rows: [selfRow({ vote: foreignVote() }), peerOneRow({ vote: voteInfo() })] });
      assert.deepEqual(ids(partitionRows(peers, reply, ME).active), [SELF_FULL, PEER_FULL]);
    });
  });

  describe("a member was excluded", () => {
    it("its peer row leaves the active list and the row of the reply goes to excluded", () => {
      const peers = [peer(SELF_FULL, { self: true }), peer(PEER_FULL)];
      const gone = peerOneRow({ status: "excluded", excluded: excludedInfo() });
      const parts = partitionRows(peers, membersReply({ rows: [selfRow(), gone] }), ME);
      assert.deepEqual(ids(parts.active), [SELF_FULL]);
      assert.deepEqual(parts.excluded, [gone]);
    });

    it("an excluded member that is not among the peers is still in excluded", () => {
      const gone = extraRow({ status: "excluded", excluded: excludedInfo({ how: "left", voters: [EXTRA_FULL] }) });
      const parts = partitionRows([peer(SELF_FULL, { self: true })], membersReply({ rows: [selfRow(), gone] }), ME);
      assert.deepEqual(ids(parts.active), [SELF_FULL]);
      assert.deepEqual(parts.excluded, [gone]);
    });

    it("excluded keeps the order of the reply", () => {
      const a = extraRow({ status: "excluded", excluded: excludedInfo() });
      const b = peerTwoRow({ status: "excluded", excluded: excludedInfo() });
      const parts = partitionRows([], membersReply({ rows: [selfRow(), b, a] }), ME);
      assert.deepEqual(ids(parts.excluded), [PEER2_FULL, EXTRA_FULL]);
    });
  });

  describe("a node is not in the member list", () => {
    it("its row of the reply goes to unlisted and its peer row leaves the active list", () => {
      const peers = [peer(SELF_FULL, { self: true }), peer(PEER2_FULL)];
      const stray = peerTwoRow({ status: "unlisted" });
      const parts = partitionRows(peers, membersReply({ rows: [selfRow(), stray] }), ME);
      assert.deepEqual(ids(parts.active), [SELF_FULL]);
      assert.deepEqual(parts.unlisted, [stray]);
    });

    it("also when the node is not among the peers; the order of the reply is kept", () => {
      const a = extraRow({ status: "unlisted", online: false });
      const b = peerTwoRow({ status: "unlisted" });
      const parts = partitionRows([], membersReply({ rows: [selfRow(), b, a] }), ME);
      assert.deepEqual(ids(parts.unlisted), [PEER2_FULL, EXTRA_FULL]);
    });
  });

  describe("a member the peer list does not have", () => {
    it("a minimal row after the peers in the active list", () => {
      const peers = [peer(SELF_FULL, { self: true })];
      const parts = partitionRows(peers, membersReply({ rows: [selfRow(), extraRow()] }), ME);
      assert.deepEqual(ids(parts.active), [SELF_FULL, EXTRA_FULL]);
      const minimal = parts.active[1];
      assert.equal(minimal.id, "dddd4444");
      assert.equal(minimal.name, "old-laptop");
      assert.equal(minimal.online, true);
      assert.equal(!!minimal.self, false);
    });
  });

  describe("a peer the reply does not know", () => {
    it("stays in the active list", () => {
      const peers = [peer(SELF_FULL, { self: true }), peer(PEER_FULL)];
      const parts = partitionRows(peers, membersReply({ rows: [selfRow()] }), ME);
      assert.deepEqual(ids(parts.active), [SELF_FULL, PEER_FULL]);
    });
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

  describe("banned by votes", () => {
    it("lists the date and both voters", () => {
      const row = extraRow({ status: "excluded", excluded: excludedInfo({ how: "votes", voters: [SELF_FULL, PEER_FULL] }) });
      assert.equal(excludedRowLine(row, names, "UTC"), "old-laptop · banned on 19 Oct · by finch-unwary and peer-one");
    });

    it("a nameless member is shown by its short id", () => {
      const row = peerTwoRow({ status: "excluded", excluded: excludedInfo({ how: "votes", voters: [SELF_FULL, PEER_FULL] }) });
      assert.equal(excludedRowLine(row, names, "UTC"), "cccc3333 · banned on 19 Oct · by finch-unwary and peer-one");
    });

    // NOTE: one voter happens in a network of two (need = 1); the wording is "by {a}".
    it("a single voter: no \"and\"", () => {
      const row = extraRow({ status: "excluded", excluded: excludedInfo({ how: "votes", voters: [PEER_FULL] }) });
      assert.equal(excludedRowLine(row, names, "UTC"), "old-laptop · banned on 19 Oct · by peer-one");
    });

    // NOTE: a voter that is in neither the peers nor the list — the first 8 characters of the id are the likely fallback.
    it("a voter nobody knows is shown by the start of its id", () => {
      const row = extraRow({ status: "excluded", excluded: excludedInfo({ how: "votes", voters: [SELF_FULL, "e".repeat(64)] }) });
      assert.equal(excludedRowLine(row, names, "UTC"), "old-laptop · banned on 19 Oct · by finch-unwary and eeeeeeee");
    });
  });

  describe("left on their own", () => {
    it("says that the member left the network", () => {
      const row = extraRow({ status: "excluded", excluded: excludedInfo({ how: "left", voters: [EXTRA_FULL] }) });
      assert.equal(excludedRowLine(row, names, "UTC"), "old-laptop · left the network on 19 Oct");
    });
  });
});
