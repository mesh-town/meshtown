/* `members-view.ts`, part 2: what a member row shows (`rowDecor`) and what its `⋯` menu offers (`menuItems`).
 * (Lookups, grouping, dates: `members-view.test.mjs`; the role items: `members-rows.managed.test.mjs`.)
 *
 * Contract fixed by these tests:
 *  - `rowDecor(peer, m, me)` -> `{ badge, lines, mainAction }`
 *      `peer`: the `state.peers` item (or a minimal row); `m`: its `MemberRow`, or null/undefined when members are not known;
 *      `me`: `id_full` of this node. `badge`: `{ text, tone }` or null. `lines`: strings that REPLACE the usual meta text of the row
 *      (`[]` = keep the usual text). `mainAction`: "unblock" | null; null = the row keeps its usual action ("Use as exit", ...).
 *      Own row and a missing `m` always give `{ badge: null, lines: [], mainAction: null }`. The only decoration is a block of mine.
 *  - `menuItems(row, ctx)` -> `MenuItem[]`, `ctx = { me, meAdmin? }`; `row` is a `MemberRow` (null/undefined/excluded -> `[]`).
 *      An item: `{ key, label, danger?, disabled?, title? }`. There are NO separator items.
 *      The unlisted row has a single item: "block", or "unblock" when it is already blocked by me. A pending row: "block".
 *  - The unlisted rows are drawn by `UnlistedList.vue`, so `rowDecor` is not defined for them (no tests). */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { menuItems, rowDecor } from "./members-view.ts";
import { SELF_FULL } from "../testing/section.mjs";
import { excludedInfo, extraRow, pendingRow, peerOneRow, peerTwoRow, selfRow } from "../testing/members-fixtures.mjs";

const ME = SELF_FULL;
const NEUTRAL = { badge: null, lines: [], mainAction: null };
const BLOCKED = { text: "Blocked on this computer", tone: "grey" };
const peerOf = (row) => ({ id: row.id, id_full: row.id_full, name: row.name, self: row.self, online: row.online });
const ctx = { me: ME, meAdmin: true };

describe("rowDecor", () => {
  describe("an ordinary member", () => {
    it("changes nothing", () => {
      const row = peerOneRow();
      assert.deepEqual(rowDecor(peerOf(row), row, ME), NEUTRAL);
    });
  });

  describe("members are not known", () => {
    it("changes nothing", () => {
      assert.deepEqual(rowDecor(peerOf(peerOneRow()), undefined, ME), NEUTRAL);
      assert.deepEqual(rowDecor(peerOf(peerOneRow()), null, ME), NEUTRAL);
    });
  });

  describe("a row that is not a member", () => {
    it("changes nothing, even with a block of mine", () => {
      const row = pendingRow({ blocked_by_me: true });
      assert.deepEqual(rowDecor(peerOf(row), row, ME), NEUTRAL);
    });
  });

  describe("it is blocked by me", () => {
    const row = peerOneRow({ blocked_by_me: true, denied_by_me: "blocked" });

    it("a grey \"Blocked on this computer\" mark", () => {
      assert.deepEqual(rowDecor(peerOf(row), row, ME).badge, BLOCKED);
    });

    it("says that it can not connect to me or use me as an exit", () => {
      assert.deepEqual(rowDecor(peerOf(row), row, ME).lines, ["Can't connect to you or open sites through you"]);
    });

    it("the main action is \"unblock\"", () => {
      assert.equal(rowDecor(peerOf(row), row, ME).mainAction, "unblock");
    });
  });

  describe("my own row", () => {
    it("a block of mine on me: nothing is shown", () => {
      const row = selfRow({ blocked_by_me: true });
      assert.deepEqual(rowDecor(peerOf(row), row, ME), NEUTRAL);
    });

    it("recognised by the peer flag even if the row has not been marked", () => {
      const row = selfRow({ self: false, blocked_by_me: true });
      assert.deepEqual(rowDecor({ ...peerOf(row), self: true }, row, ME), NEUTRAL);
    });

    it("recognised by the id even if the peer has not been marked", () => {
      const row = selfRow({ self: false, blocked_by_me: true });
      assert.deepEqual(rowDecor({ ...peerOf(row), self: false }, row, ME), NEUTRAL);
    });
  });
});

describe("menuItems", () => {
  describe("an ordinary member, I am an admin", () => {
    it("make admin, remove (dangerous), block", () => {
      assert.deepEqual(keys(menuItems(peerOneRow(), ctx)), ["make-admin", "exclude", "block"]);
      assert.deepEqual(view(menuItems(peerOneRow(), ctx)).filter((i) => i.danger).map((i) => i.key), ["exclude"]);
    });
  });

  describe("it is blocked by me", () => {
    it("block is disabled (\"Already blocked\"), remove works", () => {
      const items = view(menuItems(peerOneRow({ blocked_by_me: true }), ctx));
      assert.deepEqual(items.find((i) => i.key === "block"), item("block", "Block on this computer…", { disabled: true, title: "Already blocked" }));
      assert.equal(items.find((i) => i.key === "exclude").disabled, false);
    });
  });

  describe("it is offline", () => {
    it("the same items as an online one: no removal from the list", () => {
      assert.deepEqual(keys(menuItems(peerTwoRow({ online: false, last_seen_sec: 9999999 }), ctx)), ["make-admin", "exclude", "block"]);
    });
  });

  describe("it is not in the member list", () => {
    it("block only", () => {
      assert.deepEqual(view(menuItems(extraRow({ status: "unlisted" }), ctx)), [BLOCK]);
    });

    it("already blocked: the only item is \"Unblock\"", () => {
      const row = extraRow({ status: "unlisted", blocked_by_me: true });
      assert.deepEqual(view(menuItems(row, ctx)), [item("unblock", "Unblock")]);
    });
  });

  describe("it waits to join", () => {
    it("block only", () => {
      assert.deepEqual(view(menuItems(pendingRow(), ctx)), [BLOCK]);
    });
  });

  describe("my own row", () => {
    it("starts with rename and ends with leave the network (dangerous)", () => {
      const items = view(menuItems(selfRow(), { me: ME }));
      assert.deepEqual(items, [RENAME, LEAVE]);
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
});

// ---------------------------------------------------------------- helpers

const item = (key, label, over = {}) => ({ key, label, danger: false, disabled: false, title: "", ...over });
const BLOCK = item("block", "Block on this computer…");
const RENAME = item("rename", "Rename");
const LEAVE = item("leave", "Leave network…", { danger: true });

const keys = (items) => items.map((it) => it.key);

// Menu items in a comparable form: flags as booleans, the tooltip as a string.
function view(items) {
  return items.map((it) => ({ key: it.key, label: it.label, danger: it.danger === true, disabled: it.disabled === true, title: it.title || "" }));
}
