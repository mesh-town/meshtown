/* `members-view.ts`, part 2: what a member row shows (`rowDecor`) and what its `⋯` menu offers (`menuItems`).
 * (Lookups, grouping, dates: `members-view.test.mjs`.)
 *
 * Contract fixed by these tests:
 *  - `rowDecor(peer, m, me, names?)` -> `{ badge, lines, mainAction }`
 *      `peer`: the `state.peers` item (or a minimal row); `m`: its `MemberRow`, or null/undefined when members are not known;
 *      `me`: `id_full` of this node; `names`: the `displayNames` object (id_full -> name or short id) used for "{by}"
 *      (without it the proposer's name cannot be found); an id missing there is shown as its first 8 chars.
 *      `badge`: `{ text, tone }` or null; tone "warn" (yellow) | "grey". The mark goes after the name.
 *      `lines`: strings that REPLACE the usual meta text of the row (`[]` = keep the usual text).
 *      `mainAction`: "confirm" | "unblock" | null. null = the row keeps its usual action ("Use as exit", ...); "unblock" and
 *      "confirm" replace it (the exit button is hidden).
 *      Own row (`peer.self` or `m.id_full === me`) and a missing `m` always give `{ badge: null, lines: [], mainAction: null }`.
 *      Rule for the numbers: "more" = `m.vote.need - m.vote.votes`.
 *  - `menuItems(row, ctx)` -> `MenuItem[]`, `ctx = { me }`; `row` is a `MemberRow` (null/undefined/excluded -> `[]`).
 *      An item: `{ key, label, danger?, disabled?, title? }` — the very shape `MemberMenu.vue` takes as `items`; `key` is one of
 *      "block" | "unblock" | "exclude" | "forget" | "unforget" | "leave" | "rename" | "waiting" | "withdraw". `danger`/`disabled` are booleans when set; `title` is a
 *      string only on a disabled item (the tooltip that says why). There are NO separator items (the menu draws its own divider).
 *      The row where I proposed the ban (`vote.mine` and `vote.proposed_by === ctx.me`) has ONE enabled item, "withdraw"
 *      ("Withdraw proposal"); a row where I only confirmed somebody else's proposal (`vote.mine`, another proposer) keeps ONE disabled "waiting".
 *      The unlisted row has a single item: "block", or "unblock" (label "Unblock") when it is already blocked by me.
 *      An offline member that is not me and has no round of mine gets "forget" ("Remove from network…", "Confirm removal…" when somebody
 *      else's round is pending) after the usual items; an online member never gets it. My waiting round: one item "unforget"
 *      ("Withdraw proposal"); a forgotten row with my vote: one item "unforget" ("Undo removal"); a forgotten row of others: none.
 *  - The unlisted rows are drawn by `UnlistedList.vue`, so `rowDecor` is not defined for them (no tests). */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { menuItems, rowDecor } from "./members-view.ts";
import { PEER2_FULL, PEER_FULL, SELF_FULL } from "../testing/section.mjs";
import { AT_UNIX, excludedInfo, extraRow, foreignVote, forgetInfo, peerOneRow, peerTwoRow, selfRow, voteInfo } from "../testing/members-fixtures.mjs";

const ME = SELF_FULL;
const NAMES = { [SELF_FULL]: "finch-unwary", [PEER_FULL]: "peer-one", [PEER2_FULL]: "cccc3333" };
const NEUTRAL = { badge: null, lines: [], mainAction: null };
const PROPOSED = { text: "Ban proposed", tone: "warn" };
const BLOCKED = { text: "Blocked on this computer", tone: "grey" };
const peerOf = (row) => ({ id: row.id, id_full: row.id_full, name: row.name, self: row.self, online: row.online });

describe("rowDecor", () => {
  describe("an ordinary member", () => {
    it("changes nothing", () => {
      const row = peerOneRow();
      assert.deepEqual(rowDecor(peerOf(row), row, ME, NAMES), NEUTRAL);
    });
  });

  describe("members are not known", () => {
    it("changes nothing", () => {
      assert.deepEqual(rowDecor(peerOf(peerOneRow()), undefined, ME, NAMES), NEUTRAL);
      assert.deepEqual(rowDecor(peerOf(peerOneRow()), null, ME, NAMES), NEUTRAL);
    });
  });

  describe("I proposed banning it", () => {
    const row = peerOneRow({ vote: voteInfo(), denied_by_me: "voted_by_me" });

    it("a yellow \"Ban proposed\" mark", () => {
      assert.deepEqual(rowDecor(peerOf(row), row, ME, NAMES).badge, PROPOSED);
    });

    it("says that I wait for one more member, and that it is blocked here", () => {
      assert.deepEqual(rowDecor(peerOf(row), row, ME, NAMES).lines, [
        "You proposed banning it · waiting for 1 more member to confirm",
        "Blocked on this computer",
      ]);
    });

    it("counts the missing votes from the vote itself", () => {
      const slow = peerOneRow({ vote: voteInfo({ votes: 1, need: 3 }), denied_by_me: "voted_by_me" });
      assert.equal(rowDecor(peerOf(slow), slow, ME, NAMES).lines[0], "You proposed banning it · waiting for 2 more members to confirm");
    });

    it("there is nothing to confirm for me", () => {
      assert.notEqual(rowDecor(peerOf(row), row, ME, NAMES).mainAction, "confirm");
    });

    describe("and it is also blocked by me", () => {
      const both = peerOneRow({ vote: voteInfo(), blocked_by_me: true, denied_by_me: "voted_by_me" });

      it("the mark is still \"Ban proposed\"", () => {
        assert.deepEqual(rowDecor(peerOf(both), both, ME, NAMES).badge, PROPOSED);
      });

      it("\"Blocked on this computer\" is said once", () => {
        const lines = rowDecor(peerOf(both), both, ME, NAMES).lines;
        assert.equal(lines.filter((line) => line === "Blocked on this computer").length, 1, JSON.stringify(lines));
        assert.equal(lines.length, 2);
      });

      // NOTE: no action is defined here, so "unblock" is not offered.
      it("no \"Unblock\" and no \"Confirm\"", () => {
        const action = rowDecor(peerOf(both), both, ME, NAMES).mainAction;
        assert.notEqual(action, "unblock");
        assert.notEqual(action, "confirm");
      });
    });
  });

  describe("another member proposed banning it", () => {
    const row = peerTwoRow({ online: true, vote: foreignVote() });

    it("a yellow \"Ban proposed\" mark", () => {
      assert.deepEqual(rowDecor(peerOf(row), row, ME, NAMES).badge, PROPOSED);
    });

    it("names the proposer and says that one more confirmation is needed", () => {
      assert.deepEqual(rowDecor(peerOf(row), row, ME, NAMES).lines, ["Proposed by peer-one · needs 1 more confirmation"]);
    });

    it("a nameless proposer is shown by the short id", () => {
      const byNameless = peerOneRow({ vote: foreignVote({ proposed_by: PEER2_FULL, voters: [PEER2_FULL] }) });
      assert.deepEqual(rowDecor(peerOf(byNameless), byNameless, ME, NAMES).lines, ["Proposed by cccc3333 · needs 1 more confirmation"]);
    });

    it("counts the missing confirmations from the vote itself", () => {
      const slow = peerTwoRow({ vote: foreignVote({ votes: 1, need: 3 }) });
      assert.deepEqual(rowDecor(peerOf(slow), slow, ME, NAMES).lines, ["Proposed by peer-one · needs 2 more confirmations"]);
    });

    it("the main action is \"confirm\"", () => {
      assert.equal(rowDecor(peerOf(row), row, ME, NAMES).mainAction, "confirm");
    });
  });

  describe("another member proposed removing it", () => {
    const row = peerOneRow({ online: false, forget: forgetInfo() });

    it("a \"Removal proposed\" mark", () => {
      assert.equal(rowDecor(peerOf(row), row, ME, NAMES).badge.text, "Removal proposed");
    });

    it("there is nothing to confirm in the row itself: the main action is empty", () => {
      assert.equal(rowDecor(peerOf(row), row, ME, NAMES).mainAction, null);
    });

    it("a ban proposal on the same row wins: the mark says \"Ban proposed\"", () => {
      const both = peerOneRow({ online: false, forget: forgetInfo(), vote: voteInfo() });
      assert.deepEqual(rowDecor(peerOf(both), both, ME, NAMES).badge, PROPOSED);
    });
  });

  describe("it is blocked by me", () => {
    const row = peerOneRow({ blocked_by_me: true, denied_by_me: "blocked" });

    it("a grey \"Blocked on this computer\" mark", () => {
      assert.deepEqual(rowDecor(peerOf(row), row, ME, NAMES).badge, BLOCKED);
    });

    it("says that it can not connect to me or use me as an exit", () => {
      assert.deepEqual(rowDecor(peerOf(row), row, ME, NAMES).lines, ["Can't connect to you or open sites through you"]);
    });

    it("the main action is \"unblock\"", () => {
      assert.equal(rowDecor(peerOf(row), row, ME, NAMES).mainAction, "unblock");
    });
  });

  describe("my own row", () => {
    it.each([
      ["a vote of another member on me", selfRow({ vote: foreignVote() })],
      ["a block of mine on me", selfRow({ blocked_by_me: true })],
      ["a vote of mine on me", selfRow({ vote: voteInfo() })],
    ])("%s: nothing is shown", (_name, row) => {
      assert.deepEqual(rowDecor(peerOf(row), row, ME, NAMES), NEUTRAL);
    });

    it("recognised by the peer flag even if the row has not been marked", () => {
      const row = selfRow({ self: false, vote: foreignVote() });
      assert.deepEqual(rowDecor({ ...peerOf(row), self: true }, row, ME, NAMES), NEUTRAL);
    });

    it("recognised by the id even if the peer has not been marked", () => {
      const row = selfRow({ self: false, vote: foreignVote() });
      assert.deepEqual(rowDecor({ ...peerOf(row), self: false }, row, ME, NAMES), NEUTRAL);
    });
  });
});

describe("menuItems", () => {
  const ctx = { me: ME };

  describe("an ordinary member", () => {
    it("block, then ban (dangerous)", () => {
      assert.deepEqual(view(menuItems(peerOneRow(), ctx)), [BLOCK, BAN]);
    });
  });

  describe("I proposed banning it", () => {
    it("the only item: \"Withdraw proposal\", enabled", () => {
      const row = peerOneRow({ vote: voteInfo(), denied_by_me: "voted_by_me" });
      assert.deepEqual(view(menuItems(row, ctx)), [WITHDRAW]);
    });

    it("also when it is blocked by me", () => {
      const row = peerOneRow({ vote: voteInfo(), blocked_by_me: true });
      assert.deepEqual(view(menuItems(row, ctx)), [WITHDRAW]);
    });

    it("also when the proposal carries a reason", () => {
      const row = peerOneRow({ vote: voteInfo({ reason: "lost laptop" }) });
      assert.deepEqual(view(menuItems(row, ctx)), [WITHDRAW]);
    });

    it("there is no separate \"Waiting…\" item: the row line already says it", () => {
      const row = peerOneRow({ vote: voteInfo() });
      assert.equal(menuItems(row, ctx).some((it) => it.key === "waiting"), false);
    });
  });

  describe("I confirmed a proposal of another member", () => {
    const row = peerTwoRow({ online: true, vote: foreignVote({ proposed_by: PEER_FULL, voters: [PEER_FULL, ME], mine: true }) });

    it("the only item: \"Waiting for another member to confirm\", disabled", () => {
      assert.deepEqual(view(menuItems(row, ctx)), [WAITING]);
    });

    it("no \"Withdraw proposal\": the proposal is not mine", () => {
      assert.equal(menuItems(row, ctx).some((it) => it.key === "withdraw"), false);
    });
  });

  describe("another member proposed banning it", () => {
    it("ban is disabled with the reason, block works", () => {
      const row = peerTwoRow({ online: true, vote: foreignVote() });
      assert.deepEqual(view(menuItems(row, ctx)), [BLOCK, { ...BAN, disabled: true, title: "Ban already proposed" }]);
    });
  });

  describe("it is blocked by me", () => {
    it("block is disabled (\"Already blocked\"), ban works", () => {
      const row = peerOneRow({ blocked_by_me: true });
      assert.deepEqual(view(menuItems(row, ctx)), [{ ...BLOCK, disabled: true, title: "Already blocked" }, BAN]);
    });
  });

  describe("it is offline and nobody proposed removing it", () => {
    it("the usual items and then \"Remove from network…\"", () => {
      assert.deepEqual(keys(menuItems(peerTwoRow(), ctx)), ["block", "exclude", "forget"]);
      assert.equal(labelOf(menuItems(peerTwoRow(), ctx), "forget"), "Remove from network…");
    });

    it("\"Ban from network…\" stays next to it", () => {
      assert.equal(labelOf(menuItems(peerTwoRow(), ctx), "exclude"), "Ban from network…");
    });

    it("a ban proposed by another member keeps its disabled ban item and adds the removal", () => {
      const items = menuItems(peerTwoRow({ vote: foreignVote() }), ctx);
      assert.deepEqual(keys(items), ["block", "exclude", "forget"]);
      assert.equal(items[1].disabled, true);
    });

    it("blocked by me: block is disabled, removal is offered", () => {
      const items = menuItems(peerOneRow({ online: false, blocked_by_me: true }), ctx);
      assert.deepEqual(keys(items), ["block", "exclude", "forget"]);
    });
  });

  describe("it is online", () => {
    it("there is no removal item", () => {
      assert.equal(keys(menuItems(peerOneRow({ online: true }), ctx)).includes("forget"), false);
    });

    it("also when its last seen time is old (the row says it is online)", () => {
      assert.equal(keys(menuItems(peerOneRow({ online: true, last_seen_sec: 900000 }), ctx)).includes("forget"), false);
    });
  });

  describe("another member proposed removing it", () => {
    it("the removal item says \"Confirm removal…\"", () => {
      const items = menuItems(peerTwoRow({ forget: forgetInfo() }), ctx);
      assert.equal(labelOf(items, "forget"), "Confirm removal…");
      assert.deepEqual(keys(items), ["block", "exclude", "forget"]);
    });
  });

  describe("I proposed removing it and it waits for one more member", () => {
    const row = peerTwoRow({ forget: forgetInfo({ mine: true, proposed_by: "aaaa1111", voters: ["aaaa1111"] }), denied_by_me: "removed_by_me" });

    it("the only item: \"Withdraw proposal\" (unforget)", () => {
      assert.deepEqual(menuItems(row, ctx).map((it) => [it.key, it.label]), [["unforget", "Withdraw proposal"]]);
    });

    it("no removal item next to it", () => {
      assert.equal(keys(menuItems(row, ctx)).includes("forget"), false);
    });
  });

  describe("it is removed from the member list", () => {
    it("my vote is in: the only item is \"Undo removal\" (unforget)", () => {
      const row = peerOneRow({ status: "forgotten", online: false, denied_by_me: "removed_by_me", forget: forgetInfo({ votes: 2, mine: true, at_unix: AT_UNIX }) });
      assert.deepEqual(menuItems(row, ctx).map((it) => [it.key, it.label]), [["unforget", "Undo removal"]]);
    });

    it("the vote is not mine: no menu", () => {
      const row = peerOneRow({ status: "forgotten", online: false, forget: forgetInfo({ votes: 2, mine: false, at_unix: AT_UNIX }) });
      assert.deepEqual(menuItems(row, ctx), []);
    });
  });

  describe("it is not in the member list", () => {
    it("block only: it can not be banned", () => {
      assert.deepEqual(view(menuItems(extraRow({ status: "unlisted" }), ctx)), [BLOCK]);
    });

    it("already blocked: the only item is \"Unblock\"", () => {
      const row = extraRow({ status: "unlisted", blocked_by_me: true });
      assert.deepEqual(view(menuItems(row, ctx)), [item("unblock", "Unblock")]);
    });
  });

  describe("my own row", () => {
    it("rename, then leave the network (dangerous)", () => {
      assert.deepEqual(view(menuItems(selfRow(), ctx)), [RENAME, LEAVE]);
    });

    it("a vote on me changes nothing", () => {
      assert.deepEqual(view(menuItems(selfRow({ vote: foreignVote() }), ctx)), [RENAME, LEAVE]);
    });
  });

  describe("no menu", () => {
    it("an excluded member has none", () => {
      const row = extraRow({ status: "excluded", excluded: excludedInfo() });
      assert.deepEqual(menuItems(row, ctx), []);
    });

    it("members are not known: none", () => {
      assert.deepEqual(menuItems(null, ctx), []);
      assert.deepEqual(menuItems(undefined, ctx), []);
    });
  });

  describe("which items are dangerous", () => {
    it("only \"Ban from network…\" and \"Leave network…\" are marked", () => {
      const all = [peerOneRow(), selfRow(), extraRow({ status: "unlisted" }), peerTwoRow({ online: true, blocked_by_me: true }), peerOneRow({ vote: voteInfo() })].flatMap((row) => view(menuItems(row, ctx)));
      const dangerous = all.filter((item) => item.danger).map((item) => item.label);
      assert.deepEqual([...new Set(dangerous)].sort(), ["Ban from network…", "Leave network…"]);
    });
  });
});

// ---------------------------------------------------------------- helpers

const item = (key, label, over = {}) => ({ key, label, danger: false, disabled: false, title: "", ...over });
const BLOCK = item("block", "Block on this computer…");
const BAN = item("exclude", "Ban from network…", { danger: true });
const WAITING = item("waiting", "Waiting for another member to confirm", { disabled: true });
const WITHDRAW = item("withdraw", "Withdraw proposal");
const RENAME = item("rename", "Rename");
const LEAVE = item("leave", "Leave network…", { danger: true });

const keys = (items) => items.map((it) => it.key);
const labelOf = (items, key) => (items.find((it) => it.key === key) || {}).label;

// Menu items in a comparable form: flags as booleans, the tooltip as a string.
function view(items) {
  return items.map((it) => ({ key: it.key, label: it.label, danger: it.danger === true, disabled: it.disabled === true, title: it.title || "" }));
}
