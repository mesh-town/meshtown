/* The "how sites open right now" panel: the text and buttons come from the `panel` prop. */

import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import StatePanel from "./StatePanel.vue";

describe("StatePanel", () => {
  it("draws nothing without a panel", () => {
    expect(mountPanel(null).html()).not.toContain("route-state");
  });

  it("the main text in #route-state, the caption next to it, a region with a name", () => {
    const w = mountPanel({ level: "green", main: "Sites go via peer-one", sub: "Direct connection", actions: [] });
    expect(w.get('[aria-label="How sites open right now"]').attributes("role")).toBe("region");
    expect(w.get("#route-state").text()).toBe("Sites go via peer-one");
    expect(w.get(".state-sub").text()).toBe("Direct connection");
    expect(w.findAll("button")).toHaveLength(0);
  });

  it("\"Choose another exit\" emits pick-other", async () => {
    const w = mountPanel({ level: "yellow", main: "m", sub: "", actions: ["pick-other"] });
    await w.get("button").trigger("click");
    expect(w.emitted("pick-other")).toHaveLength(1);
  });

  it("\"Open sites as usual\" emits direct, and there is no other button", async () => {
    const w = mountPanel({ level: "yellow", main: "m", sub: "", actions: ["direct"] });
    expect(w.findAll("button").map((b) => b.text())).toEqual(["Open sites as usual"]);
    await w.get("button").trigger("click");
    expect(w.emitted("direct")).toHaveLength(1);
  });
});

describe("StatePanel > member ring", () => {
  describe("there is a ring", () => {
    it("instead of a dot .ring-mark is drawn", () => {
      const w = mountWithRing();
      expect(w.find(".state-dot").exists()).toBe(false);
      expect(w.findAll(".ring-mark")).toHaveLength(1);
    });

    it("the ring has a path per member", () => {
      expect(mountWithRing().findAll(".ring-mark path")).toHaveLength(2);
    });

    it("the ring center is taken from ring.center", () => {
      const w = mountWithRing();
      expect(w.get(".ring-center-main").text()).toBe("1 of 2");
      expect(w.get(".ring-center-sub").text()).toBe("online");
    });

    it("the ul.ring-legend legend comes after .state-sub, an li per role", () => {
      const w = mountWithRing();
      const items = w.findAll("ul.ring-legend > li");
      expect(items.map((li) => li.classes())).toEqual([
        expect.arrayContaining(["ring-legend-online"]),
        expect.arrayContaining(["ring-legend-offline"]),
      ]);
      expect(items.map((li) => li.text())).toEqual(["Online: 1", "Offline: 1"]);
      const sub = w.get(".state-sub").element;
      expect(sub.compareDocumentPosition(w.get("ul.ring-legend").element) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    });

    it("the main text, caption and buttons stayed in place", async () => {
      const w = mountWithRing({ actions: ["pick-other"] });
      expect(w.get("#route-state").text()).toBe("Sites go directly");
      expect(w.get(".state-sub").text()).toBe("Direct connection");
      await w.get("button").trigger("click");
      expect(w.emitted("pick-other")).toHaveLength(1);
    });

    it("empty set: a dashed ring and an empty legend", () => {
      const ring = { members: [], center: { main: "only you", sub: "" }, legend: [] };
      const w = mountWithRing({}, ring);
      expect(w.find("circle.ring-empty").exists()).toBe(true);
      expect(w.findAll("ul.ring-legend > li")).toHaveLength(0);
    });
  });

  describe("no ring", () => {
    it("ring = null — the old dot, no rings or legend", () => {
      const w = mount(StatePanel, { props: { panel: panelOf(), ring: null } });
      expect(w.find(".state-dot.green").exists()).toBe(true);
      expect(w.find(".ring-mark").exists()).toBe(false);
      expect(w.find(".ring-legend").exists()).toBe(false);
    });

    it("prop not passed — also a dot", () => {
      const w = mountPanel(panelOf());
      expect(w.find(".state-dot.green").exists()).toBe(true);
      expect(w.find(".ring-mark").exists()).toBe(false);
    });
  });
});

function mountPanel(panel) {
  return mount(StatePanel, { props: { panel } });
}

function panelOf(extra = {}) {
  return { level: "green", main: "Sites go directly", sub: "Direct connection", actions: [], ...extra };
}

/** Returns a panel with a ring: one online and one not. */
function mountWithRing(panelExtra = {}, ring = ringOf()) {
  return mount(StatePanel, { props: { panel: panelOf(panelExtra), ring } });
}

function ringOf() {
  return {
    members: [
      { role: "online", label: "aa", idFull: "aa_full" },
      { role: "offline", label: "bb", idFull: "bb_full" },
    ],
    center: { main: "1 of 2", sub: "online" },
    legend: [
      { role: "online", text: "Online: 1" },
      { role: "offline", text: "Offline: 1" },
    ],
  };
}
