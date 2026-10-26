/* The tab bar: roles, the selected tab, the "unsaved" label on "Settings", scrolling the active one. */

import { afterEach, describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";
import TabsNav from "./TabsNav.vue";

const NAMES = ["Home", "Networks", "Site checks", "Diagnostics", "Log", "Settings"];
let wrapper;

afterEach(() => {
  wrapper.unmount();
  delete HTMLElement.prototype.scrollIntoView;
});

describe("TabsNav", () => {
  describe("markup", () => {
    it("a tablist with the network logo and six role=tab tabs in page order, the second is Networks", () => {
      wrapper = mountNav({ active: "home" });
      expect(wrapper.get("[role=tablist]").get(".brand-name").text()).toBe("Mesh Town");
      expect(wrapper.findAll("[role=tab]").map((t) => t.text())).toEqual(NAMES);
    });

    it("there is no Ports tab, and #tab-peers is the Networks link", () => {
      wrapper = mountNav({ active: "home" });
      expect(wrapper.find("#tab-ports").exists()).toBe(false);
      expect(wrapper.get("#tab-peers").text()).toBe("Networks");
      expect(wrapper.get("#tab-peers").attributes("href")).toBe("#peers");
    });

    it("a tab is a link to the section hash and controls its panel", () => {
      wrapper = mountNav({ active: "home" });
      const log = wrapper.get("#tab-log");
      expect(log.attributes("href")).toBe("#log");
      expect(log.attributes("aria-controls")).toBe("sec-log");
    });
  });

  describe("active tab", () => {
    it("aria-selected and class active only on it", () => {
      wrapper = mountNav({ active: "peers" });
      const selected = wrapper.findAll("[role=tab]").filter((t) => t.attributes("aria-selected") === "true");
      expect(selected.map((t) => t.text())).toEqual(["Networks"]);
      expect(wrapper.findAll(".tab.active")).toHaveLength(1);
      expect(wrapper.get("#tab-peers").classes()).toContain("active");
    });

    it("after a change it scrolls into the visible part of the bar", async () => {
      const scroll = vi.fn();
      HTMLElement.prototype.scrollIntoView = scroll;
      wrapper = mountNav({ active: "home" });
      await wrapper.setProps({ active: "log" });
      await new Promise((resolve) => setTimeout(resolve, 0));
      expect(scroll).toHaveBeenCalledWith({ block: "nearest", inline: "nearest" });
    });

    it("no scrollIntoView — changing the tab does not crash", async () => {
      wrapper = mountNav({ active: "home" });
      await wrapper.setProps({ active: "log" });
      await new Promise((resolve) => setTimeout(resolve, 0));
      expect(wrapper.get("#tab-log").classes()).toContain("active");
    });
  });

  describe("there are unsaved edits", () => {
    it("\"Settings\" has the has-dirty class and a hint, the others do not", () => {
      wrapper = mountNav({ active: "home", dirty: true });
      const settings = wrapper.get("#tab-settings");
      expect(settings.classes()).toContain("has-dirty");
      expect(settings.attributes("title")).toBe("You have unsaved changes");
      expect(wrapper.findAll(".has-dirty")).toHaveLength(1);
    });

    it("changing the active tab does not lose the label", async () => {
      wrapper = mountNav({ active: "settings", dirty: true });
      await wrapper.setProps({ active: "log" });
      expect(wrapper.get("#tab-settings").classes()).toContain("has-dirty");
      await wrapper.setProps({ active: "settings" });
      expect(wrapper.get("#tab-settings").classes()).toContain("has-dirty");
    });

    it("edits cleared — no label and no hint", async () => {
      wrapper = mountNav({ active: "home", dirty: true });
      await wrapper.setProps({ dirty: false });
      expect(wrapper.get("#tab-settings").classes()).not.toContain("has-dirty");
      expect(wrapper.get("#tab-settings").attributes("title")).toBeUndefined();
    });
  });
});

function mountNav(props) {
  return mount(TabsNav, { props, attachTo: document.body });
}
