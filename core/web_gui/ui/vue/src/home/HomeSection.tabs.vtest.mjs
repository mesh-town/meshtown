/* The tabs of "Home": "Main" and "Setup"; the open tab by mode and by click. */

import { nextTick } from "vue";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { baseConfig, baseState, mountSection } from "../testing/section.mjs";
import HomeSection from "./HomeSection.vue";

const tabs = (w) => w.findAll('[role="tablist"][aria-label] [role="tab"]');
const tab = (w, id) => w.get("#home-tab-" + id);
const labelled = (w, id) => w.get('[role="tabpanel"]').attributes("aria-labelledby") === "home-tab-" + id;
const clickButton = async (w, text) => {
  await w.findAll("button").find((b) => b.text() === text).trigger("click");
  await flushPromises();
};
const stepOf = (w) => w.get("#wizard-screen").attributes("data-step");
const clickById = async (w, selector) => {
  await w.get(selector).trigger("click");
  await flushPromises();
};
let mounted;

beforeEach(() => localStorage.clear());
afterEach(() => mounted.wrapper.unmount());

describe("HomeSection > tabs, no network", () => {
  it("two tabs with aria, the setup is open; \"Main\" shows the name field", async () => {
    const { wrapper } = mountHome(baseState({ network: null, peers: [] }));
    expect(tabs(wrapper).map((t) => t.text())).toEqual(["Overview", "Setup"]);
    expect(tab(wrapper, "wizard").attributes("aria-selected")).toBe("true");
    expect(tab(wrapper, "main").attributes("aria-selected")).toBe("false");
    expect(tab(wrapper, "main").attributes("aria-controls")).toBe("home-tabpanel");
    expect(wrapper.find("#home-start-title").exists()).toBe(true);
    expect(wrapper.find("#field-node-name").exists()).toBe(false);
    await tab(wrapper, "main").trigger("click");
    expect(wrapper.find("#field-node-name").exists()).toBe(true);
    expect(wrapper.find("#home-start-title").exists()).toBe(false);
    expect(labelled(wrapper, "main")).toBe(true);
  });

  it("the → arrow from \"Setup\" wraps around to \"Main\", focus is on it", async () => {
    const { wrapper } = mountHome(baseState({ network: null, peers: [] }));
    await tab(wrapper, "wizard").trigger("keydown", { key: "ArrowRight" });
    await nextTick();
    expect(tab(wrapper, "main").attributes("aria-selected")).toBe("true");
    expect(document.activeElement).toBe(tab(wrapper, "main").element);
    await tab(wrapper, "main").trigger("keydown", { key: "ArrowLeft" });
    expect(tab(wrapper, "wizard").attributes("aria-selected")).toBe("true");
  });
});

describe("HomeSection > tabs, network exists, no label", () => {
  it("\"Main\" is open; \"Setup\" shows the second screen, \"Next\" leads to the exit screen, \"Finish\" returns to \"Main\"", async () => {
    const { wrapper } = mountHome(baseState());
    expect(tab(wrapper, "main").attributes("aria-selected")).toBe("true");
    await tab(wrapper, "wizard").trigger("click");
    expect(stepOf(wrapper)).toBe("choices");
    expect(wrapper.get("#wizard-joined").text()).toContain("✓ You joined the network");
    await clickById(wrapper, "#wizard-next");
    expect(stepOf(wrapper)).toBe("exit");
    expect(tab(wrapper, "wizard").attributes("aria-selected")).toBe("true");
    expect(wrapper.get("#wizard-next").text()).toBe("Finish");
    await clickById(wrapper, "#wizard-next");
    expect(tab(wrapper, "main").attributes("aria-selected")).toBe("true");
    expect(wrapper.find(".wizard-card").exists()).toBe(false);
    expect(localStorage.getItem("meshtown.homeSetup")).toBe("done");
  });

  it("\"Open step-by-step setup\" on \"Main\" opens \"Setup\" at the scope question, a remembered answer does not skip it", async () => {
    localStorage.setItem("meshtown.homeSetup", "done");
    localStorage.setItem("meshtown.proxyScope", "browser");
    const { wrapper } = mountHome(baseState({ socks: { enabled: true, listen: "127.0.0.1:1080" } }));
    expect(tab(wrapper, "main").attributes("aria-selected")).toBe("true");
    await clickButton(wrapper, "Open step-by-step setup");
    expect(tab(wrapper, "wizard").attributes("aria-selected")).toBe("true");
    expect(stepOf(wrapper)).toBe("choices");
    expect(wrapper.get("#scope-browser").attributes("aria-pressed"), "the remembered answer is shown pressed").toBe("true");
    expect(wrapper.get("#scope-system").attributes("aria-pressed")).toBe("false");
    expect(localStorage.getItem("meshtown.homeSetup")).toBe("browser");
  });
});

describe("HomeSection > tabs, no snapshot", () => {
  it("instead of tabs — only \"Loading\"", () => {
    const { wrapper } = mountHome(null);
    expect(wrapper.find('[role="tablist"]').exists()).toBe(false);
  });
});

function mountHome(state) {
  return (mounted = mountSection(HomeSection, { config: baseConfig(), state }));
}
