/* The sub-tabs of a network tab: Members, Shared services (ports), Device settings. A click or an arrow key writes
 * the address `#peers/<slot>[/<sub>]`; the shell reads the choice back, so the component itself only shows `active`. */

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import NetSubTabs from "./NetSubTabs.vue";

const NET_B = "k7q2xa";
const ALL = ["members", "services", "device"];
const IN_APP = ["members", "device"];
let wrapper;

beforeEach(() => { location.hash = ""; });
afterEach(() => {
  wrapper.unmount();
  location.hash = "";
});

describe("NetSubTabs", () => {
  describe("markup", () => {
    it("a labelled tablist with three role=tab buttons in order", () => {
      const row = setup({ active: "members" });
      expect(row.get("#net-subtabs").attributes("role")).toBe("tablist");
      expect(row.get("#net-subtabs").attributes("aria-label")).toBe("Network sections");
      expect(tabLabels(row)).toEqual(["Members", "Shared services (ports)", "Device settings"]);
    });

    it("every tab has its own id and controls the shared panel", () => {
      const row = setup({ active: "members" });
      for (const sub of ALL) {
        expect(row.get("#net-sub-" + sub).attributes("role"), sub).toBe("tab");
        expect(row.get("#net-sub-" + sub).attributes("aria-controls"), sub).toBe("net-subpanel");
      }
    });

    it("looks like the tabs of Home", () => {
      const row = setup({ active: "members" });
      expect(row.get("#net-subtabs").classes()).toContain("home-tabs");
      expect(row.get("#net-sub-members").classes()).toContain("home-tab");
    });
  });

  describe("the active tab", () => {
    it("is the only one selected, active and in the tab order", () => {
      const row = setup({ active: "services" });
      expect(selected(row)).toEqual(["Shared services (ports)"]);
      expect(row.get("#net-sub-services").classes()).toContain("active");
      expect(row.findAll(".home-tab.active")).toHaveLength(1);
      expect(row.get("#net-sub-services").attributes("tabindex")).toBe("0");
      expect(row.get("#net-sub-members").attributes("tabindex")).toBe("-1");
      expect(row.get("#net-sub-device").attributes("tabindex")).toBe("-1");
    });

    it("follows the prop", async () => {
      const row = setup({ active: "members" });
      await row.setProps({ active: "device" });
      expect(selected(row)).toEqual(["Device settings"]);
      expect(row.get("#net-sub-device").attributes("tabindex")).toBe("0");
    });
  });

  describe("a click", () => {
    it("click writes #peers/<slot>[/sub]: services and device get a tail", async () => {
      const row = setup({ active: "members", slot: NET_B });
      await row.get("#net-sub-services").trigger("click");
      expect(location.hash).toBe("#peers/" + NET_B + "/services");
      await row.get("#net-sub-device").trigger("click");
      expect(location.hash).toBe("#peers/" + NET_B + "/device");
    });

    it("members writes the net alone, without a tail", async () => {
      const row = setup({ active: "services", slot: NET_B });
      await row.get("#net-sub-members").trigger("click");
      expect(location.hash).toBe("#peers/" + NET_B);
    });

    it("the first network is written as main", async () => {
      const row = setup({ active: "members", slot: "main" });
      await row.get("#net-sub-services").trigger("click");
      expect(location.hash).toBe("#peers/main/services");
    });

    it("does not change the shown tab by itself: the address is the only source", async () => {
      const row = setup({ active: "members" });
      await row.get("#net-sub-services").trigger("click");
      expect(selected(row)).toEqual(["Members"]);
    });
  });

  describe("the keyboard", () => {
    it("arrows move focus and open: → goes to the next tab", async () => {
      const row = setup({ active: "members", slot: NET_B });
      await row.get("#net-sub-members").trigger("keydown", { key: "ArrowRight" });
      expect(location.hash).toBe("#peers/" + NET_B + "/services");
      expect(document.activeElement).toBe(row.get("#net-sub-services").element);
    });

    it("← goes to the previous tab", async () => {
      const row = setup({ active: "device", slot: NET_B });
      await row.get("#net-sub-device").trigger("keydown", { key: "ArrowLeft" });
      expect(location.hash).toBe("#peers/" + NET_B + "/services");
      expect(document.activeElement).toBe(row.get("#net-sub-services").element);
    });

    it("→ from the last tab wraps to Members, which has no tail", async () => {
      const row = setup({ active: "device", slot: NET_B });
      await row.get("#net-sub-device").trigger("keydown", { key: "ArrowRight" });
      expect(location.hash).toBe("#peers/" + NET_B);
      expect(document.activeElement).toBe(row.get("#net-sub-members").element);
    });

    it("← from the first tab wraps to the last one", async () => {
      const row = setup({ active: "members", slot: NET_B });
      await row.get("#net-sub-members").trigger("keydown", { key: "ArrowLeft" });
      expect(location.hash).toBe("#peers/" + NET_B + "/device");
      expect(document.activeElement).toBe(row.get("#net-sub-device").element);
    });

    it("Home goes to the first tab", async () => {
      const row = setup({ active: "device", slot: NET_B });
      await row.get("#net-sub-device").trigger("keydown", { key: "Home" });
      expect(location.hash).toBe("#peers/" + NET_B);
      expect(document.activeElement).toBe(row.get("#net-sub-members").element);
    });

    it("End goes to the last tab", async () => {
      const row = setup({ active: "members", slot: NET_B });
      await row.get("#net-sub-members").trigger("keydown", { key: "End" });
      expect(location.hash).toBe("#peers/" + NET_B + "/device");
      expect(document.activeElement).toBe(row.get("#net-sub-device").element);
    });

    it("another key does nothing", async () => {
      const row = setup({ active: "members", slot: NET_B });
      await row.get("#net-sub-members").trigger("keydown", { key: "a" });
      expect(location.hash).toBe("");
    });
  });

  describe("in the phone app", () => {
    it("app: two tabs, Members and Device settings", () => {
      const row = setup({ active: "members", subs: IN_APP });
      expect(tabLabels(row)).toEqual(["Members", "Device settings"]);
      expect(row.find("#net-sub-services").exists()).toBe(false);
    });

    it("the arrows go round the two tabs", async () => {
      const row = setup({ active: "members", slot: NET_B, subs: IN_APP });
      await row.get("#net-sub-members").trigger("keydown", { key: "ArrowRight" });
      expect(location.hash).toBe("#peers/" + NET_B + "/device");
      await row.get("#net-sub-device").trigger("keydown", { key: "ArrowRight" });
      expect(location.hash).toBe("#peers/" + NET_B);
    });

    it("End is Device settings", async () => {
      const row = setup({ active: "members", slot: NET_B, subs: IN_APP });
      await row.get("#net-sub-members").trigger("keydown", { key: "End" });
      expect(location.hash).toBe("#peers/" + NET_B + "/device");
    });
  });
});

function setup({ active, slot = "main", subs = ALL }) {
  wrapper = mount(NetSubTabs, { props: { slot, active, subs }, attachTo: document.body });
  return wrapper;
}

function tabLabels(row) {
  return row.findAll("[role=tab]").map((t) => t.text());
}

function selected(row) {
  return row.findAll("[role=tab]").filter((t) => t.attributes("aria-selected") === "true").map((t) => t.text());
}
