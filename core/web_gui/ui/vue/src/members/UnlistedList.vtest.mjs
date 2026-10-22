/* Contract of `members/UnlistedList.vue` (written before the component: red until it exists).
 *
 * Component: the group under the Members list with the nodes that knocked but are not in the member list (`status = "unlisted"`).
 * They cannot be removed (they are not members); the only thing to do with them is to block / unblock them on this computer.
 * Props: `rows: { id: string; id_full: string; name: string | null; blocked_by_me: boolean }[]` (rows of `GET /api/members`).
 * Emits: `pick(key, idFull)` with key `"block"` or `"unblock"` — the parent opens the dialog (block) or calls the API (unblock).
 * Markup: `details#unlisted-list` with `summary` "Not in the member list (N)"; per row an `li` with the name (or the short `id` when
 *   there is no name), the short id in `.peer-code`, the badge `.peer-badge` "Not a member", and a `MemberMenu` (toggle
 *   `button[data-member-menu=<id_full>]`) with ONE item: `block` "Block on this computer…" or, for a blocked node, `unblock` "Unblock".
 *   No rows — nothing is rendered. */

import { afterEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import UnlistedList from "./UnlistedList.vue";

const STRAY = { id: "dddd4444", id_full: "D".repeat(64), name: "old-laptop", blocked_by_me: false };
const NAMELESS_BLOCKED = { id: "eeee5555", id_full: "E".repeat(64), name: null, blocked_by_me: true };

let wrapper;
afterEach(() => wrapper.unmount());

describe("UnlistedList", () => {
  describe("with two rows", () => {
    it("is a <details> whose summary counts them", () => {
      const { page } = setup([STRAY, NAMELESS_BLOCKED]);
      expect(page.summary()).toBe("Not in the member list (2)");
      expect(wrapper.get("details#unlisted-list").exists()).toBe(true);
    });

    it("shows the name, or the short id for a node without a name, the code and the badge", () => {
      const { page } = setup([STRAY, NAMELESS_BLOCKED]);
      expect(page.rows()).toHaveLength(2);
      expect(page.row(0).text()).toContain("old-laptop");
      expect(page.row(0).get(".peer-code").text()).toBe("dddd4444");
      expect(page.row(0).get(".peer-badge").text()).toBe("Not a member");
      expect(page.row(1).text()).toContain("eeee5555");
      expect(page.row(1).get(".peer-badge").text()).toBe("Not a member");
    });
  });

  describe("without rows", () => {
    it("renders nothing", () => {
      setup([]);
      expect(wrapper.find("details").exists()).toBe(false);
    });
  });

  describe("the menu of a node that is not blocked", () => {
    it("has the single item \"Block on this computer…\"", async () => {
      const { page } = setup([STRAY]);
      await page.openMenu(STRAY.id_full);
      expect(page.menuItems()).toEqual([["block", "Block on this computer…"]]);
    });

    it("choosing it emits pick(\"block\", id_full)", async () => {
      const { page } = setup([STRAY]);
      await page.openMenu(STRAY.id_full);
      await page.chooseItem("block");
      expect(wrapper.emitted("pick")).toEqual([["block", STRAY.id_full]]);
    });
  });

  describe("the menu of a blocked node", () => {
    it("has the single item \"Unblock\"", async () => {
      const { page } = setup([NAMELESS_BLOCKED]);
      await page.openMenu(NAMELESS_BLOCKED.id_full);
      expect(page.menuItems()).toEqual([["unblock", "Unblock"]]);
    });

    it("choosing it emits pick(\"unblock\", id_full)", async () => {
      const { page } = setup([NAMELESS_BLOCKED]);
      await page.openMenu(NAMELESS_BLOCKED.id_full);
      await page.chooseItem("unblock");
      expect(wrapper.emitted("pick")).toEqual([["unblock", NAMELESS_BLOCKED.id_full]]);
    });
  });
});

function setup(rows) {
  wrapper = mount(UnlistedList, { props: { rows }, attachTo: document.body });
  const page = {
    summary: () => wrapper.get("summary").text(),
    rows: () => wrapper.findAll("#unlisted-list li"),
    row: (i) => page.rows()[i],
    openMenu: (idFull) => wrapper.get('button[data-member-menu="' + idFull + '"]').trigger("click"),
    menuItems: () => wrapper.findAll('[role="menuitem"]').map((i) => [i.attributes("data-member-item"), i.text()]),
    chooseItem: (key) => wrapper.get('[data-member-item="' + key + '"]').trigger("click"),
  };
  return { page };
}
