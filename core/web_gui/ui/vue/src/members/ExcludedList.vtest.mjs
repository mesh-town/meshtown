/* Contract of `members/ExcludedList.vue` (written before the component: red until it exists).
 *
 * Component: the collapsed group under the Members list with the nodes that are no longer in the network.
 * Props: `lines: string[]` — one ready-made line per node, e.g. "old-laptop · left the network on 19 Oct" (built by the parent).
 * Markup: `details#excluded-list` (collapsed by default; the class `removed` is styling) with `summary` "No longer in the network (N)"
 *   and one `li` per line. No lines — the component renders nothing at all. The open / closed state is the user's: it
 *   survives a re-render with new lines (the user's toggle is reported by the browser's `toggle` event). */

import { afterEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import ExcludedList from "./ExcludedList.vue";

const LINES = ["old-laptop · left the network on 19 Oct", "peer-one · banned on 19 Oct · by peer-two and finch-unwary"];

let wrapper;
afterEach(() => wrapper.unmount());

describe("ExcludedList", () => {
  describe("with two lines", () => {
    it("is a collapsed <details> whose summary counts them", () => {
      setup(LINES);
      const details = wrapper.get("details#excluded-list");
      expect(details.attributes("open")).toBeUndefined();
      expect(details.get("summary").text()).toBe("No longer in the network (2)");
    });

    it("lists the lines in the given order", () => {
      setup(LINES);
      expect(wrapper.findAll("#excluded-list li").map((li) => li.text())).toEqual(LINES);
    });
  });

  describe("with one line", () => {
    it("says (1)", () => {
      setup([LINES[0]]);
      expect(wrapper.get("summary").text()).toBe("No longer in the network (1)");
    });
  });

  describe("without lines", () => {
    it("renders nothing", () => {
      setup([]);
      expect(wrapper.find("details").exists()).toBe(false);
      expect(wrapper.text()).toBe("");
    });

    it("appears when a line arrives", async () => {
      setup([]);
      await wrapper.setProps({ lines: [LINES[0]] });
      expect(wrapper.get("summary").text()).toBe("No longer in the network (1)");
    });
  });

  describe("the lines change", () => {
    it("the count follows", async () => {
      setup([LINES[0]]);
      await wrapper.setProps({ lines: LINES });
      expect(wrapper.get("summary").text()).toBe("No longer in the network (2)");
      expect(wrapper.findAll("#excluded-list li")).toHaveLength(2);
    });

    it("an open list stays open", async () => {
      setup([LINES[0]]);
      const details = wrapper.get("details").element;
      details.open = true;
      details.dispatchEvent(new Event("toggle"));
      await wrapper.setProps({ lines: LINES });
      expect(wrapper.get("details").element.open).toBe(true);
    });
  });
});

function setup(lines) {
  wrapper = mount(ExcludedList, { props: { lines }, attachTo: document.body });
}
