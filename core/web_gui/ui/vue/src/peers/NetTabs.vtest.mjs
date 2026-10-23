import { afterEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import NetTabs from "./NetTabs.vue";

const FRIENDS = { slot: "main", name: "friends", sub: "· 2 online", title: "friends — 2 of 3 members online", votes: 0 };
const DEVICES = { slot: "k7q2xa", name: "my devices", sub: "· 1 online", title: "my devices — 1 of 1 members online", votes: 0 };

let wrapper;
afterEach(() => wrapper && wrapper.unmount());

describe("NetTabs", () => {
  describe("no networks", () => {
    it("shows only the \"+\" tab", () => {
      const row = render({ tabs: [] });
      expect(row.findAll("[role=tab]")).toHaveLength(1);
      expect(plusTab(row).exists()).toBe(true);
    });
  });

  describe("two networks", () => {
    it("is a tab list with a tab per network and the \"+\"", () => {
      const row = render({ tabs: [FRIENDS, DEVICES] });
      expect(row.find("[role=tablist]").exists()).toBe(true);
      expect(row.findAll("[role=tab]")).toHaveLength(3);
    });

    it("links each tab to its own address", () => {
      const row = render({ tabs: [FRIENDS, DEVICES] });
      expect(tab(row, "friends").attributes("href")).toBe("#peers/main");
      expect(tab(row, "my devices").attributes("href")).toBe("#peers/k7q2xa");
      expect(plusTab(row).attributes("href")).toBe("#peers/add");
    });

    it("marks only the active tab as selected", () => {
      const row = render({ tabs: [FRIENDS, DEVICES], active: "k7q2xa" });
      expect(tab(row, "friends").attributes("aria-selected")).toBe("false");
      expect(tab(row, "my devices").attributes("aria-selected")).toBe("true");
      expect(plusTab(row).attributes("aria-selected")).toBe("false");
    });

    it("marks the \"+\" as selected when it is active", () => {
      const row = render({ tabs: [FRIENDS, DEVICES], active: "add" });
      expect(plusTab(row).attributes("aria-selected")).toBe("true");
      expect(tab(row, "friends").attributes("aria-selected")).toBe("false");
    });
  });

  describe("online network", () => {
    it("shows the name, the online tail and the title", () => {
      const row = render({ tabs: [FRIENDS] });
      expect(tab(row, "friends").text()).toContain("friends");
      expect(tab(row, "friends").text()).toContain("· 2 online");
      expect(tab(row, "friends").attributes("title")).toBe("friends — 2 of 3 members online");
    });
  });

  describe("network without connection", () => {
    it("shows \"· no connection\" instead of the count", () => {
      const down = { ...DEVICES, sub: "· no connection", title: "my devices — no connection" };
      const row = render({ tabs: [FRIENDS, down] });
      expect(tab(row, "my devices").text()).toContain("· no connection");
      expect(tab(row, "my devices").text()).not.toContain("online");
      expect(tab(row, "my devices").attributes("title")).toBe("my devices — no connection");
    });
  });

  describe("requests to confirm", () => {
    it("shows a circle with the number and a title", () => {
      const row = render({ tabs: [FRIENDS, { ...DEVICES, votes: 2 }] });
      const circle = tab(row, "my devices").get("[title=\"2 to confirm\"]");
      expect(circle.text()).toBe("2");
    });

    it("shows no circle when there are none", () => {
      const row = render({ tabs: [FRIENDS, DEVICES] });
      expect(row.find("[title$=\"to confirm\"]").exists()).toBe(false);
    });

    it("keeps the circle on a tab without connection", () => {
      const down = { ...DEVICES, sub: "· no connection", title: "my devices — no connection", votes: 1 };
      const row = render({ tabs: [FRIENDS, down] });
      expect(tab(row, "my devices").get("[title=\"1 to confirm\"]").text()).toBe("1");
    });
  });

  describe("same names", () => {
    it("tells the networks apart by the short id in the label", () => {
      const a = { ...FRIENDS, name: "friends · 3f2a" };
      const b = { ...DEVICES, name: "friends · 9c01" };
      const row = render({ tabs: [a, b] });
      const tabs = row.findAll("[role=tab]");
      expect(tabs[0].text()).toContain("· 3f2a");
      expect(tabs[1].text()).toContain("· 9c01");
      expect(tabs[0].text()).not.toContain("9c01");
    });
  });

  describe("the \"+\" tab", () => {
    it("has the title \"Add network\"", () => {
      expect(plusTab(render({ tabs: [FRIENDS, DEVICES] })).attributes("title")).toBe("Add network");
    });

    it("at the limit explains why more cannot be added", () => {
      const row = render({ tabs: [FRIENDS, DEVICES], full: true });
      expect(plusTab(row).attributes("title")).toBe("Mesh Town can be in up to 8 networks at once");
    });
  });

  describe("a network being joined", () => {
    it("is an inactive label with \"· joining…\" and its own title", () => {
      const row = render({ tabs: [FRIENDS], joining: [{ name: "New network" }] });
      const joining = row.findAll("span").find((s) => s.text().includes("New network"));
      expect(joining, "a label for the incoming network").toBeTruthy();
      expect(joining.text()).toContain("· joining…");
      expect(joining.attributes("title") ?? joining.element.closest("[title]")?.getAttribute("title")).toBe("Joining the network…");
    });

    it("is not a link", () => {
      const row = render({ tabs: [FRIENDS], joining: [{ name: "New network" }] });
      expect(row.findAll("a").some((a) => a.text().includes("New network"))).toBe(false);
    });

    it("does not count as a tab with an address", () => {
      const row = render({ tabs: [FRIENDS], joining: [{ name: "New network" }] });
      expect(row.findAll("a[href^=\"#peers/\"]")).toHaveLength(2);
    });
  });
});

function render(props) {
  wrapper = mount(NetTabs, { props: { tabs: [], active: "main", joining: [], full: false, ...props } });
  return wrapper;
}

function tab(row, name) {
  const found = row.findAll("[role=tab]").find((t) => t.text().includes(name));
  expect(found, `a tab named ${name}`).toBeTruthy();
  return found;
}

function plusTab(row) {
  return row.get("a[href=\"#peers/add\"]");
}
