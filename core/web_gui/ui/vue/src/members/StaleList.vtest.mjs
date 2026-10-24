/* `members/StaleList.vue`: the collapsed group under the Members list with members that have been offline for over a week
 * (or were never seen). Props: `rows` (`MemberRow`s), `me` (my id_full). Markup: `details#stale-list` with a summary and a
 * row per member: the name, the short id, the "Last online …" line, the "Removal proposed" badge while a removal circle waits,
 * and the `⋯` menu. There is no bulk action: each member is removed with its own menu item. The component reports
 * `pick(key, idFull)` and does nothing else. No rows - nothing is rendered. */

import { afterEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import StaleList from "./StaleList.vue";
import { forgetInfo, staleRow } from "../testing/members-fixtures.mjs";
import { SELF_FULL } from "../testing/section.mjs";

const DAY = 86400;

let wrapper;
afterEach(() => wrapper.unmount());

describe("StaleList", () => {
  describe("without rows", () => {
    it("hidden when empty", () => {
      const { wrapper: w } = setup([]);
      expect(w.find("#stale-list").exists()).toBe(false);
      expect(w.text()).toBe("");
    });
  });

  describe("with rows", () => {
    it("summary counts rows", () => {
      setup([staleRow("old-desktop", 9 * DAY), staleRow("ghost-key", null)]);
      const details = wrapper.get("details#stale-list");
      expect(details.attributes("open"), "collapsed by default").toBeUndefined();
      expect(details.get("summary").text()).toBe("Offline for over a week (2)");
      expect(wrapper.findAll("li")).toHaveLength(2);
    });

    it("summary says one for a single row", () => {
      setup([staleRow("old-desktop", 9 * DAY)]);
      expect(wrapper.get("summary").text()).toBe("Offline for over a week (1)");
    });

    it("row shows last online and menu", async () => {
      const row = staleRow("old-desktop", 9 * DAY);
      setup([row]);
      expect(wrapper.get(".peer-name").text()).toBe("old-desktop");
      expect(wrapper.get(".peer-line").text()).toBe("Last online 9 d ago");
      await openMenu(row);
      expect(menuLabels()).toContain("Remove from network…");
    });

    it("never seen row says so", () => {
      setup([staleRow("ghost-key", null)]);
      expect(wrapper.get(".peer-line").text()).toBe("Never seen online");
    });

    it("row without a name shows the short id", () => {
      const row = { ...staleRow("ghost-key", null), name: null };
      setup([row]);
      expect(wrapper.get(".peer-name").text()).toBe(row.id);
    });

    it("every row has its own menu", () => {
      const rows = [staleRow("old-desktop", 9 * DAY), staleRow("ghost-key", null)];
      setup(rows);
      for (const row of rows) expect(wrapper.find('button[data-member-menu="' + row.id_full + '"]').exists(), row.name).toBe(true);
    });
  });

  describe("a removal round of someone else waits", () => {
    it("row shows removal proposed badge", () => {
      setup([staleRow("old-desktop", 9 * DAY)], { forget: forgetInfo() });
      expect(wrapper.get(".peer-badge").text()).toBe("Removal proposed");
    });

    it("menu offers to confirm the removal", async () => {
      const row = { ...staleRow("old-desktop", 9 * DAY), forget: forgetInfo() };
      setup([row]);
      await openMenu(row);
      expect(menuLabels()).toContain("Confirm removal…");
      expect(menuLabels()).not.toContain("Remove from network…");
    });
  });

  describe("no round", () => {
    it("row has no badge", () => {
      setup([staleRow("old-desktop", 9 * DAY)]);
      expect(wrapper.find(".peer-badge").exists()).toBe(false);
    });
  });

  describe("picking a menu item", () => {
    it("pick emits forget with id", async () => {
      const row = staleRow("old-desktop", 9 * DAY);
      setup([row]);
      await openMenu(row);
      await wrapper.get('[role="menu"] [data-member-item="forget"]').trigger("click");
      expect(wrapper.emitted("pick")).toEqual([["forget", row.id_full]]);
    });

    it("pick names the row that was picked", async () => {
      const rows = [staleRow("old-desktop", 9 * DAY), staleRow("ghost-key", null)];
      setup(rows);
      await openMenu(rows[1]);
      await wrapper.get('[role="menu"] [data-member-item="forget"]').trigger("click");
      expect(wrapper.emitted("pick")).toEqual([["forget", rows[1].id_full]]);
    });
  });
});

function setup(rows, over = null) {
  const list = over ? rows.map((r) => ({ ...r, ...over })) : rows;
  wrapper = mount(StaleList, { props: { rows: list, me: SELF_FULL }, attachTo: document.body });
  return { wrapper };
}

const openMenu = (row) => wrapper.get('button[data-member-menu="' + row.id_full + '"]').trigger("click");
const menuLabels = () => wrapper.findAll('[role="menu"] [role="menuitem"]').map((i) => i.text());
