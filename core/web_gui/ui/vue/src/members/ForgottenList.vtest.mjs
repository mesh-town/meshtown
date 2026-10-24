/* `members/ForgottenList.vue`: the collapsed group under the Members list with nodes removed from the member list, and with
 * members whose removal waits for one more vote started by me (the round is hidden from my own list). Props: `rows` (the
 * group), `all` (every row of the reply, to turn short voter ids into names), `me` (my id_full). Markup: `details#forgotten-list`
 * with a summary and a row per node: the name, the short id, one line (waiting / "Removed by A and B on 19 Oct") and the `⋯` menu.
 * Reports `pick(key, idFull)` and does nothing else. No rows - nothing is rendered. */

import { afterEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import ForgottenList from "./ForgottenList.vue";
import { AT_UNIX, forgetInfo, memberRow, peerOneRow, selfRow, staleRow } from "../testing/members-fixtures.mjs";
import { SELF_FULL } from "../testing/section.mjs";

const DAY = 86400;
const ME_CODE = "aaaa1111";
const ONE_CODE = "bbbb2222";

let wrapper;
afterEach(() => wrapper.unmount());

describe("ForgottenList", () => {
  describe("without rows", () => {
    it("hidden when empty", () => {
      setup([]);
      expect(wrapper.find("#forgotten-list").exists()).toBe(false);
      expect(wrapper.text()).toBe("");
    });
  });

  describe("my removal round waits for a second vote", () => {
    it("waiting row offers withdraw", async () => {
      const row = waiting();
      setup([row]);
      expect(wrapper.get("summary").text()).toBe("Removed from the member list (1)");
      expect(wrapper.get(".peer-line").text()).toBe("Waiting for one more member to confirm");
      await openMenu(row);
      expect(menuLabels()).toEqual(["Withdraw proposal"]);
    });

    it("withdraw emits unforget with id", async () => {
      const row = waiting();
      setup([row]);
      await openMenu(row);
      await wrapper.get('[role="menu"] [data-member-item="unforget"]').trigger("click");
      expect(wrapper.emitted("pick")).toEqual([["unforget", row.id_full]]);
    });
  });

  describe("removed by me and another member", () => {
    it("forgotten mine offers undo", async () => {
      const row = removedByMe();
      setup([row]);
      await openMenu(row);
      expect(menuLabels()).toEqual(["Undo removal"]);
      await wrapper.get('[role="menu"] [data-member-item="unforget"]').trigger("click");
      expect(wrapper.emitted("pick")).toEqual([["unforget", row.id_full]]);
    });

    it("line names voters", () => {
      setup([removedByMe()]);
      expect(wrapper.get(".peer-line").text()).toBe("Removed by finch-unwary and peer-one on 19 Oct");
    });

    it("line falls back to the short id of an unknown voter", () => {
      setup([removedByMe({ voters: [ME_CODE, "9999ffff"] })]);
      expect(wrapper.get(".peer-line").text()).toBe("Removed by finch-unwary and 9999ffff on 19 Oct");
    });
  });

  describe("removed by others, without my vote", () => {
    it("forgotten not mine has no menu", () => {
      const row = removedByMe({ mine: false, voters: [ONE_CODE, "cccc3333"] });
      setup([row]);
      expect(wrapper.find('button[data-member-menu="' + row.id_full + '"]').exists()).toBe(false);
    });
  });

  describe("several rows", () => {
    it("summary counts rows", () => {
      setup([waiting(), removedByMe()]);
      expect(wrapper.get("details#forgotten-list").attributes("open"), "collapsed by default").toBeUndefined();
      expect(wrapper.get("summary").text()).toBe("Removed from the member list (2)");
      expect(wrapper.findAll("li")).toHaveLength(2);
    });
  });
});

// A member whose removal I proposed and nobody has confirmed: it stays `member`, its round is mine and not complete.
function waiting() {
  return { ...staleRow("lost-phone", 2 * DAY), forget: forgetInfo({ votes: 1, proposed_by: ME_CODE, voters: [ME_CODE], mine: true }) };
}

// A node removed by me and peer-one on 19 Oct (voters in the order of voting).
function removedByMe(over = {}) {
  const forget = forgetInfo({ votes: 2, proposed_by: ME_CODE, voters: [ME_CODE, ONE_CODE], mine: true, at_unix: AT_UNIX, ...over });
  return { ...staleRow("old-tablet", 30 * DAY), status: "forgotten", forget };
}

function setup(rows) {
  const all = [selfRow(), peerOneRow(), memberRow(), ...rows];
  wrapper = mount(ForgottenList, { props: { rows, all, me: SELF_FULL }, attachTo: document.body });
}

const openMenu = (row) => wrapper.get('button[data-member-menu="' + row.id_full + '"]').trigger("click");
const menuLabels = () => wrapper.findAll('[role="menu"] [role="menuitem"]').map((i) => i.text());
