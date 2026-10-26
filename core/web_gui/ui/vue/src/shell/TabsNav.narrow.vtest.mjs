/* The tab bar on a narrow screen: no logo, four tabs and a "More" menu with the rare sections. */

import { afterEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { stubNarrow } from "../testing/narrow-media.mjs";
import TabsNav from "./TabsNav.vue";

let wrapper;
const setUa = (ua) => Object.defineProperty(navigator, "userAgent", { value: ua, configurable: true });

afterEach(() => {
  wrapper.unmount();
  Reflect.deleteProperty(navigator, "userAgent");
});

describe("TabsNav on a narrow screen", () => {
  describe("the strip", () => {
    it("has no network logo block", () => {
      const { nav } = setup({ active: "home" });
      expect(nav.find(".tabs-brand").exists()).toBe(false);
      expect(nav.text()).not.toContain("Mesh Town");
    });

    it("shows Home, Networks, Sites, Settings and the \"More ▾\" button", () => {
      const { nav } = setup({ active: "home" });
      expect(tabLabels(nav)).toEqual(["Home", "Networks", "Sites", "Settings"]);
      expect(moreButton(nav).text()).toBe("More ▾");
    });

    it("keeps the request counter on Networks", () => {
      const { nav } = setup({ active: "home", votes: 2 });
      expect(nav.get("#tab-peers .chip").text()).toBe("2");
    });

    it("has no sections of the menu among the main tabs", () => {
      const { nav } = setup({ active: "home" });
      for (const id of ["ports", "network", "log"]) expect(nav.find("[role=tab]#tab-" + id).exists(), id).toBe(false);
    });
  });

  describe("the \"More\" menu", () => {
    it("is closed at first", () => {
      const { nav } = setup({ active: "home" });
      expect(menuOpen(nav)).toBe(false);
    });

    it("opens on click and lists Diagnostics and Log as links to their sections", async () => {
      const { nav } = setup({ active: "home" });
      await moreButton(nav).trigger("click");
      const items = nav.get("[role=menu]").findAll("[role=menuitem]");
      expect(items.map((i) => i.text())).toEqual(["Diagnostics", "Log"]);
      expect(items.map((i) => i.attributes("id"))).toEqual(["tab-network", "tab-log"]);
      expect(items.map((i) => i.attributes("href"))).toEqual(["#network", "#log"]);
    });

    it("a second click on \"More\" closes it", async () => {
      const { nav } = setup({ active: "home" });
      await moreButton(nav).trigger("click");
      await moreButton(nav).trigger("click");
      expect(menuOpen(nav)).toBe(false);
    });

    it("a click on an item closes it", async () => {
      const { nav } = setup({ active: "home" });
      await moreButton(nav).trigger("click");
      await nav.get("#tab-network").trigger("click");
      expect(menuOpen(nav)).toBe(false);
    });

    it("a click outside closes it", async () => {
      const { nav } = setup({ active: "home" });
      await moreButton(nav).trigger("click");
      await clickOutside();
      expect(menuOpen(nav)).toBe(false);
    });

    it("a change of the active section closes it", async () => {
      const { nav } = setup({ active: "home" });
      await moreButton(nav).trigger("click");
      await nav.setProps({ active: "peers" });
      expect(menuOpen(nav)).toBe(false);
    });
  });

  describe("the active section is under \"More\"", () => {
    it("the button is active and named after the section, ordinary tabs are not active", () => {
      const { nav } = setup({ active: "log" });
      expect(moreButton(nav).text()).toBe("Log ▾");
      expect(moreButton(nav).classes()).toContain("active");
      for (const t of nav.findAll("[role=tab]").filter((t) => !t.text().includes("▾"))) {
        expect(t.classes(), t.text()).not.toContain("active");
        expect(t.attributes("aria-selected"), t.text()).not.toBe("true");
      }
    });

    it("another menu section gives its own name", () => {
      expect(moreButton(setup({ active: "network" }).nav).text()).toBe("Diagnostics ▾");
    });
  });

  describe("the active section is an ordinary tab", () => {
    it("the button stays \"More ▾\" and is not active; the tab is", () => {
      const { nav } = setup({ active: "domains" });
      expect(moreButton(nav).text()).toBe("More ▾");
      expect(moreButton(nav).classes()).not.toContain("active");
      expect(nav.get("#tab-domains").classes()).toContain("active");
    });
  });

  describe("in the phone app", () => {
    it("there is no \"More\" button and no menu, the three tabs remain", () => {
      setUa("Mozilla/5.0 (Linux; Android 14) Chrome/120 Mobile MeshTownApp/Android");
      const { nav } = setup({ active: "home" });
      expect(moreButton(nav)).toBeUndefined();
      expect(nav.find("[role=menu]").exists()).toBe(false);
      expect(tabLabels(nav)).toEqual(["Home", "Networks", "Settings"]);
    });
  });

  describe("the window is resized", () => {
    it("growing to a wide window brings back the logo and all six tabs", async () => {
      const { nav, media } = setup({ active: "home" });
      media.change(false);
      await nav.vm.$nextTick();
      expect(nav.find(".tabs-brand").exists()).toBe(true);
      expect(tabLabels(nav)).toEqual(["Home", "Networks", "Site checks", "Diagnostics", "Log", "Settings"]);
      expect(moreButton(nav)).toBeUndefined();
    });
  });
});

describe("TabsNav on a wide screen", () => {
  it("has no \"More\" button", () => {
    const { nav } = setup({ active: "home" }, false);
    expect(moreButton(nav)).toBeUndefined();
    expect(nav.find(".tabs-brand").exists()).toBe(true);
  });
});

function setup(props, narrow = true) {
  const media = stubNarrow(narrow);
  wrapper = mount(TabsNav, { props, attachTo: document.body });
  return { nav: wrapper, media };
}

/** The labels of the plain tabs (everything but the "More ▾" button). */
function tabLabels(nav) {
  return nav.findAll("[role=tab]").map((t) => t.text().replace(/\d+$/, "")).filter((t) => !t.includes("▾"));
}

/** The "More ▾" / "Log ▾" button: the first button or link of the strip whose text ends with the arrow. */
function moreButton(nav) {
  return nav.findAll("button, a, [role=tab]").find((e) => e.text().endsWith("▾"));
}

function menuOpen(nav) {
  const menu = nav.find("[role=menu]");
  return menu.exists() && !menu.attributes("hidden") && menu.element.style.display !== "none";
}

async function clickOutside() {
  for (const type of ["pointerdown", "mousedown", "mouseup", "click"]) {
    document.body.dispatchEvent(new MouseEvent(type, { bubbles: true }));
  }
  await new Promise((resolve) => setTimeout(resolve, 0));
}
