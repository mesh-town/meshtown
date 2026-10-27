/* The setup wizard as one screen at a time: where it starts, how Next, Back, Skip and the tiles move along the path,
 * how the step counter changes with the answers, the marks written to the page storage and the focus. */

import { nextTick } from "vue";
import { afterEach, describe, expect, it } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { baseConfig, baseState, mountSection } from "../testing/section.mjs";
import {
  MARK_KEY, SCOPE_KEY, brokenStorage, click, mountWizard, pressedTiles, progress, step, tiles, toGuide, unmountWizard,
} from "../testing/wizard.mjs";
import HomeSection from "./HomeSection.vue";

const MAC_SAFARI = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15";
const WITH_PROXY = { socks: { enabled: true, listen: "127.0.0.1:1080" } };
let mounted;
const mount = (opts) => (mounted = mountWizard(opts));

afterEach(() => unmountWizard(mounted));

describe("SetupWizard > start", () => {
  it("starts on the join screen without Back", () => {
    const { wrapper } = mount({ hasNet: false });
    expect(step(wrapper)).toBe("join");
    expect(progress(wrapper)).toBe("Step 1 of 6");
    expect(wrapper.find("#wizard-back").exists()).toBe(false);
    expect(wrapper.find("#wizard-next").exists(), "the join screen has its own buttons").toBe(false);
  });

  it("joining a network moves on to the scope question as step 2 of 6, with the joined line and no Back", async () => {
    const { wrapper, store } = mount({ hasNet: false });
    store.state = baseState({ ...WITH_PROXY, network: { id_hex: "net-aaaa", label: "family" } });
    await flushPromises();
    expect(step(wrapper)).toBe("scope");
    expect(progress(wrapper)).toBe("Step 2 of 6");
    expect(wrapper.get("#wizard-joined").text()).toBe("✓ You joined the network \"family\"");
    expect(wrapper.find("#wizard-back").exists()).toBe(false);
  });

  it("opens the scope question as step 2 of 6", () => {
    const { wrapper } = mount({ mark: "browser" });
    expect(step(wrapper)).toBe("scope");
    expect(progress(wrapper)).toBe("Step 2 of 6");
    expect(wrapper.get("#wizard-q").text()).toBe("Where should Mesh Town open sites?");
    expect(tiles(wrapper, "scope-seg")).toEqual(["All apps on this computer (recommended)", "Only in one browser"]);
  });

  it("shows the joined line on the first screen after joining only", async () => {
    const { wrapper } = mount({ mark: "browser", scope: "system", label: "family" });
    expect(wrapper.get("#wizard-joined").text()).toContain("✓ You joined the network");
    await click(wrapper, "#wizard-next");
    expect(wrapper.find("#wizard-joined").exists()).toBe(false);
  });

  it("a finished setup opens the scope question again instead of jumping ahead", () => {
    const { wrapper } = mount({ mark: "done" });
    expect(step(wrapper)).toBe("scope");
  });

  it("the exit mark opens the exit screen", () => {
    const { wrapper } = mount({ mark: "exit" });
    expect(step(wrapper)).toBe("exit");
    expect(progress(wrapper)).toBe("Step 6 of 6");
  });
});

describe("SetupWizard > scope", () => {
  it("system choice leads to the system question", async () => {
    const { wrapper } = mount({ mark: "browser" });
    await click(wrapper, "#scope-system");
    expect(step(wrapper)).toBe("os");
    expect(progress(wrapper)).toBe("Step 3 of 6");
  });

  it("browser choice skips the system question, step count drops to 5", async () => {
    const { wrapper } = mount({ mark: "browser" });
    await click(wrapper, "#scope-browser");
    expect(step(wrapper)).toBe("browser");
    expect(progress(wrapper)).toBe("Step 3 of 5");
  });

  it("a choice is remembered for the next time", async () => {
    const { wrapper } = mount({ mark: "browser" });
    await click(wrapper, "#scope-browser");
    expect(localStorage.getItem(SCOPE_KEY)).toBe("browser");
  });

  it("the remembered scope is pressed and Next goes on", async () => {
    const { wrapper } = mount({ mark: "browser", scope: "browser" });
    expect(pressedTiles(wrapper, "scope-seg")).toEqual(["browser"]);
    expect(wrapper.find("#wizard-skip").exists(), "an answered question has no Skip").toBe(false);
    await click(wrapper, "#wizard-next");
    expect(step(wrapper)).toBe("browser");
  });

  it("without an answer there is Skip instead of Next", () => {
    const { wrapper } = mount({ mark: "browser" });
    expect(wrapper.get("#wizard-skip").text()).toBe("Skip this step");
    expect(wrapper.find("#wizard-next").exists()).toBe(false);
    expect(pressedTiles(wrapper, "scope-seg")).toEqual([]);
  });

  it("skip goes to the exit screen", async () => {
    const { wrapper } = mount({ mark: "browser" });
    await click(wrapper, "#wizard-skip");
    expect(step(wrapper)).toBe("exit");
    expect(progress(wrapper)).toBe("Step 3 of 3");
  });

  it("skip does not remember a scope", async () => {
    const { wrapper } = mount({ mark: "browser" });
    await click(wrapper, "#wizard-skip");
    expect(localStorage.getItem(SCOPE_KEY)).toBeNull();
  });
});

describe("SetupWizard > system and browser", () => {
  it("the system of this computer is pressed and Next goes on", async () => {
    const { wrapper } = mount({ mark: "browser", scope: "system" });
    await click(wrapper, "#wizard-next");
    expect(step(wrapper)).toBe("os");
    expect(wrapper.get("#wizard-q").text()).toBe("Which system are you setting up?");
    expect(tiles(wrapper, "os-seg")).toEqual(["macOS", "Windows", "Linux", "Android", "iOS"]);
    expect(pressedTiles(wrapper, "os-seg")).toEqual(["mac"]);
  });

  it("choosing Android removes the browser question, step count is 5", async () => {
    const { wrapper } = mount({ mark: "browser", scope: "system" });
    await toGuide(wrapper, { os: "android" });
    expect(step(wrapper)).toBe("guide");
    expect(progress(wrapper)).toBe("Step 4 of 5");
  });

  it.each([
    ["mac", ["Safari", "Chrome", "Firefox", "Edge", "Other"]],
    ["windows", ["Chrome", "Edge", "Firefox", "Other"]],
    ["linux", ["Firefox", "Chrome", "Other"]],
  ])("the browser tiles of %s", async (os, names) => {
    const { wrapper } = mount({ mark: "browser", scope: "system" });
    await click(wrapper, "#wizard-next");
    await click(wrapper, "#os-" + os);
    expect(step(wrapper)).toBe("browser");
    expect(tiles(wrapper, "browser-seg")).toEqual(names);
  });

  it("the browser of this computer is pressed", async () => {
    const { wrapper } = mount({ mark: "browser", scope: "browser" });
    await click(wrapper, "#wizard-next");
    expect(pressedTiles(wrapper, "browser-seg")).toEqual(["chrome"]);
  });

  it("a browser the system does not offer falls back to Other", async () => {
    const { wrapper } = mount({ mark: "browser", scope: "system", ua: MAC_SAFARI });
    await click(wrapper, "#wizard-next");
    await click(wrapper, "#os-linux");
    expect(pressedTiles(wrapper, "browser-seg")).toEqual(["other"]);
  });

  it("choosing a browser opens the guide", async () => {
    const { wrapper } = mount({ mark: "browser", scope: "browser" });
    await toGuide(wrapper, { browser: "other" });
    expect(step(wrapper)).toBe("guide");
    expect(progress(wrapper)).toBe("Step 4 of 5");
  });
});

describe("SetupWizard > Back", () => {
  it("back returns to the previous answer kept pressed", async () => {
    const { wrapper } = mount({ mark: "browser", scope: "system" });
    await toGuide(wrapper, { browser: "firefox" });
    await click(wrapper, "#wizard-back");
    expect(step(wrapper)).toBe("browser");
    expect(pressedTiles(wrapper, "browser-seg")).toEqual(["firefox"]);
  });

  it("back through the browser question returns to the system chosen before", async () => {
    const { wrapper } = mount({ mark: "browser", scope: "system" });
    await toGuide(wrapper, { os: "windows" });
    await click(wrapper, "#wizard-back");
    await click(wrapper, "#wizard-back");
    expect(step(wrapper)).toBe("os");
    expect(pressedTiles(wrapper, "os-seg")).toEqual(["windows"]);
  });

  it("back from the browser question returns to the scope with the answer pressed", async () => {
    const { wrapper } = mount({ mark: "browser" });
    await click(wrapper, "#scope-browser");
    await click(wrapper, "#wizard-back");
    expect(step(wrapper)).toBe("scope");
    expect(pressedTiles(wrapper, "scope-seg")).toEqual(["browser"]);
  });

  it("the first screen in a network has no Back", () => {
    const { wrapper } = mount({ mark: "browser" });
    expect(wrapper.find("#wizard-back").exists()).toBe(false);
  });
});

describe("SetupWizard > exit", () => {
  it("entering exit writes the exit mark", async () => {
    const { wrapper } = mount({ mark: "browser", scope: "system" });
    await toGuide(wrapper);
    expect(localStorage.getItem(MARK_KEY), "not before the exit screen").toBe("browser");
    await click(wrapper, "#wizard-next");
    expect(step(wrapper)).toBe("exit");
    expect(localStorage.getItem(MARK_KEY)).toBe("exit");
  });

  it("the last button is Finish", () => {
    const { wrapper } = mount({ mark: "exit" });
    expect(wrapper.get("#wizard-next").text()).toBe("Finish");
  });

  it("finish writes done", async () => {
    const { wrapper } = mount({ mark: "exit" });
    await click(wrapper, "#wizard-next");
    expect(localStorage.getItem(MARK_KEY)).toBe("done");
  });

  it("finish opens overview", async () => {
    localStorage.clear();
    localStorage.setItem(MARK_KEY, "exit");
    mounted = mountSection(HomeSection, { config: baseConfig(), state: baseState(WITH_PROXY) });
    await mounted.wrapper.get("#home-tab-wizard").trigger("click");
    await click(mounted.wrapper, "#wizard-next");
    expect(mounted.wrapper.get("#home-tab-main").attributes("aria-selected")).toBe("true");
    expect(mounted.wrapper.find(".wizard-card").exists()).toBe(false);
  });
});

describe("SetupWizard > the step counter", () => {
  it("walks 2 to 6 of 6 along the all-apps path", async () => {
    const { wrapper } = mount({ mark: "browser", scope: "system" });
    const seen = [];
    for (let i = 0; i < 5; i += 1) {
      seen.push(step(wrapper) + ":" + progress(wrapper));
      if (i < 4) await click(wrapper, "#wizard-next");
    }
    expect(seen).toEqual(["scope:Step 2 of 6", "os:Step 3 of 6", "browser:Step 4 of 6", "guide:Step 5 of 6", "exit:Step 6 of 6"]);
  });
});

describe("SetupWizard > focus", () => {
  it("moves to the question after Next, not on mounting", async () => {
    const { wrapper } = mount({ mark: "browser", scope: "system" });
    expect(document.activeElement).not.toBe(wrapper.get("#wizard-q").element);
    await wrapper.get("#wizard-next").trigger("click");
    await nextTick();
    await flushPromises();
    expect(document.activeElement).toBe(wrapper.get("#wizard-q").element);
  });
});

describe("SetupWizard > proxy off", () => {
  it("proxy off shows the settings hint and three steps", async () => {
    const { wrapper } = mount({ mark: "browser", proxyOn: false });
    expect(step(wrapper)).toBe("scope");
    expect(progress(wrapper)).toBe("Step 2 of 3");
    expect(wrapper.get("#proxy-hint").text()).toContain("The browser proxy is off");
    expect(wrapper.get("#proxy-hint a").attributes("href")).toBe("#settings");
    expect(wrapper.find("#scope-seg").exists()).toBe(false);
    expect(wrapper.find("#wizard-skip").exists()).toBe(false);
    await click(wrapper, "#wizard-next");
    expect(step(wrapper)).toBe("exit");
    expect(progress(wrapper)).toBe("Step 3 of 3");
  });
});

describe("SetupWizard > a page storage that refuses to write", () => {
  it("storage errors do not stop the wizard: the scope choice", async () => {
    const { wrapper, storage } = mount({ mark: "browser", storage: brokenStorage() });
    await click(wrapper, "#scope-system");
    expect(step(wrapper)).toBe("os");
    expect(storage.data.has(SCOPE_KEY)).toBe(false);
  });

  it("storage errors do not stop the wizard: the exit mark and Finish", async () => {
    const { wrapper } = mount({ mark: "browser", scope: "system", storage: brokenStorage() });
    await toGuide(wrapper);
    await click(wrapper, "#wizard-next");
    expect(step(wrapper)).toBe("exit");
    await click(wrapper, "#wizard-next");
    expect(step(wrapper)).toBe("exit");
  });
});
