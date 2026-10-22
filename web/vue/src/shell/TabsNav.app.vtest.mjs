/* The tab bar inside the Android app: no "Site checks", "Ports", "Diagnostics", "Log" and no network name block. */

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import TabsNav from "./TabsNav.vue";

let wrapper;
const setUa = (ua) => Object.defineProperty(navigator, "userAgent", { value: ua, configurable: true });

beforeEach(() => setUa("Mozilla/5.0 (Linux; Android 14) Chrome/120 Mobile MeshTownApp/Android"));
afterEach(() => {
  wrapper.unmount();
  Reflect.deleteProperty(navigator, "userAgent");
});

describe("TabsNav in the app", () => {
  it("three tabs, the desktop-only ones are absent", () => {
    wrapper = mount(TabsNav, { props: { active: "home" }, attachTo: document.body });
    expect(wrapper.findAll("[role=tab]").map((t) => t.text())).toEqual(["Home", "Members", "Settings"]);
  });

  it("no network name block", () => {
    wrapper = mount(TabsNav, { props: { active: "home" }, attachTo: document.body });
    expect(wrapper.find(".tabs-brand").exists()).toBe(false);
  });
});
