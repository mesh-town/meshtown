/* The setup wizard as one questions page and the exit screen: where the cursor starts, how the tiles open the rows,
 * how Next, Back and Skip move between the screens, how the step counter changes with the answers, the marks written to the page storage and the focus. */

import { nextTick } from "vue";
import { afterEach, describe, expect, it } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { baseConfig, baseState, mountSection } from "../testing/section.mjs";
import {
  LINUX_FIREFOX, MARK_KEY, SCOPE_KEY, brokenStorage, click, cursor, mountWizard, pressedTiles, progress, rows, step, tiles, toGuide, unmountWizard, WINDOWS_EDGE,
} from "../testing/wizard.mjs";
import HomeSection from "./HomeSection.vue";

const LINUX_SAFARI = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15";
const WITH_PROXY = { socks: { enabled: true, listen: "127.0.0.1:1080" } };
let mounted;
const mount = (opts) => (mounted = mountWizard(opts));

afterEach(() => unmountWizard(mounted));

describe("SetupWizard > start", () => {
  it("starts on the join screen without Back", () => {
    const { wrapper } = mount({ hasNet: false });
    expect(step(wrapper)).toBe("join");
    expect(progress(wrapper)).toBe("Step 1 of 5");
    expect(wrapper.find("#wizard-back").exists()).toBe(false);
    expect(wrapper.find("#wizard-next").exists(), "the join screen has its own buttons").toBe(false);
  });

  it("joining a network moves on to the scope question as step 2 of 5, with the joined line and no Back", async () => {
    const { wrapper, store } = mount({ hasNet: false });
    store.state = baseState({ ...WITH_PROXY, network: { id_hex: "net-aaaa", label: "family" } });
    await flushPromises();
    expect(cursor(wrapper)).toBe("scope");
    expect(progress(wrapper)).toBe("Step 2 of 5");
    expect(wrapper.get("#wizard-joined").text()).toBe("✓ You joined the network \"family\"");
    expect(wrapper.find("#wizard-back").exists()).toBe(false);
  });

  it("opens the scope question as step 2 of 5", () => {
    const { wrapper } = mount({ mark: "browser" });
    expect(step(wrapper)).toBe("choices");
    expect(rows(wrapper), "no row below the open question").toEqual(["scope"]);
    expect(progress(wrapper)).toBe("Step 2 of 5");
    expect(wrapper.get("#wizard-q-scope").text()).toBe("Where should Mesh Town open sites?");
    expect(tiles(wrapper, "scope-seg")).toEqual(["All apps on this computer (recommended)", "Only in one browser"]);
  });

  it("shows the joined line on the questions page only", async () => {
    const { wrapper } = mount({ mark: "browser", scope: "system", label: "family" });
    expect(wrapper.get("#wizard-joined").text()).toContain("✓ You joined the network");
    await click(wrapper, "#wizard-next");
    expect(wrapper.find("#wizard-joined").exists()).toBe(false);
  });

  it("a remembered scope counts as answered: every row down to the instruction is open", () => {
    const { wrapper } = mount({ mark: "browser", scope: "system" });
    expect(rows(wrapper)).toEqual(["scope", "os", "guide"]);
    expect(progress(wrapper)).toBe("Step 4 of 5");
  });

  it("a finished setup opens the scope question again instead of jumping ahead", () => {
    const { wrapper } = mount({ mark: "done" });
    expect(cursor(wrapper)).toBe("scope");
  });

  it("the exit mark opens the exit screen", () => {
    const { wrapper } = mount({ mark: "exit" });
    expect(step(wrapper)).toBe("exit");
    expect(progress(wrapper)).toBe("Step 5 of 5");
  });
});

describe("SetupWizard > scope", () => {
  it("system choice opens the system question and the rows below it", async () => {
    const { wrapper } = mount({ mark: "browser" });
    await click(wrapper, "#scope-system");
    expect(rows(wrapper)).toEqual(["scope", "os", "guide"]);
    expect(progress(wrapper)).toBe("Step 4 of 5");
  });

  it("browser choice swaps the system question for the browser one, step count stays 5", async () => {
    const { wrapper } = mount({ mark: "browser" });
    await click(wrapper, "#scope-browser");
    expect(rows(wrapper)).toEqual(["scope", "browser", "guide"]);
    expect(progress(wrapper)).toBe("Step 4 of 5");
  });

  it("a choice is remembered for the next time", async () => {
    const { wrapper } = mount({ mark: "browser" });
    await click(wrapper, "#scope-browser");
    expect(localStorage.getItem(SCOPE_KEY)).toBe("browser");
  });

  it("the remembered scope is pressed and the next row is already open", async () => {
    const { wrapper } = mount({ mark: "browser", scope: "browser" });
    expect(pressedTiles(wrapper, "scope-seg")).toEqual(["browser"]);
    expect(wrapper.find("#wizard-skip").exists(), "an answered question has no Skip").toBe(false);
    expect(rows(wrapper)).toEqual(["scope", "browser", "guide"]);
  });

  it("without an answer there is Skip and no Next", () => {
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
  it("the system of this computer is pressed", async () => {
    const { wrapper } = mount({ mark: "browser", scope: "system" });
    expect(rows(wrapper)).toContain("os");
    expect(wrapper.get("#wizard-q-os").text()).toBe("Which system are you setting up?");
    expect(tiles(wrapper, "os-seg")).toEqual(["macOS", "Windows", "Linux", "Android", "iOS"]);
    expect(pressedTiles(wrapper, "os-seg")).toEqual(["mac"]);
  });

  it("the all-apps path has no browser question, whatever the system", async () => {
    const { wrapper } = mount({ mark: "browser", scope: "system" });
    for (const os of ["windows", "linux", "android", "mac"]) {
      await toGuide(wrapper, { os });
      expect(rows(wrapper), os).toEqual(["scope", "os", "guide"]);
      expect(wrapper.find("#browser-seg").exists(), os).toBe(false);
    }
    expect(progress(wrapper)).toBe("Step 4 of 5");
  });

  it.each([
    ["mac", undefined, ["Safari", "Chrome", "Firefox", "Edge", "Other"]],
    ["windows", WINDOWS_EDGE, ["Chrome", "Edge", "Firefox", "Other"]],
    ["linux", LINUX_FIREFOX, ["Firefox", "Chrome", "Other"]],
  ])("the browser tiles of %s", (os, ua, names) => {
    const { wrapper } = mount({ mark: "browser", scope: "browser", ua });
    expect(tiles(wrapper, "browser-seg")).toEqual(names);
  });

  it("the browser of this computer is pressed", async () => {
    const { wrapper } = mount({ mark: "browser", scope: "browser" });
    expect(pressedTiles(wrapper, "browser-seg")).toEqual(["chrome"]);
  });

  it("a browser the system does not offer falls back to Other", async () => {
    const { wrapper } = mount({ mark: "browser", scope: "browser", ua: LINUX_SAFARI });
    expect(pressedTiles(wrapper, "browser-seg")).toEqual(["other"]);
  });

  it("choosing a browser opens the guide", async () => {
    const { wrapper } = mount({ mark: "browser", scope: "browser" });
    await toGuide(wrapper, { browser: "other" });
    expect(cursor(wrapper)).toBe("guide");
    expect(progress(wrapper)).toBe("Step 4 of 5");
  });

  it("an answer in an upper row keeps the rows below and does not move the cursor", async () => {
    const { wrapper } = mount({ mark: "browser", scope: "system" });
    await click(wrapper, "#os-windows");
    expect(rows(wrapper)).toEqual(["scope", "os", "guide"]);
    expect(pressedTiles(wrapper, "os-seg")).toEqual(["windows"]);
  });

  it("switching the scope to one browser removes the system row and keeps the instruction", async () => {
    const { wrapper } = mount({ mark: "browser", scope: "system" });
    await click(wrapper, "#scope-browser");
    expect(rows(wrapper)).toEqual(["scope", "browser", "guide"]);
    expect(wrapper.find("#os-seg").exists()).toBe(false);
  });
});

describe("SetupWizard > Back", () => {
  it("back returns to the previous answer kept pressed", async () => {
    const { wrapper } = mount({ mark: "browser", scope: "browser" });
    await toGuide(wrapper, { browser: "firefox" });
    await click(wrapper, "#wizard-next");
    await click(wrapper, "#wizard-back");
    expect(step(wrapper)).toBe("choices");
    expect(cursor(wrapper)).toBe("guide");
    expect(pressedTiles(wrapper, "browser-seg")).toEqual(["firefox"]);
  });

  it("back through the browser question returns to the system chosen before", async () => {
    const { wrapper } = mount({ mark: "browser", scope: "system" });
    await toGuide(wrapper, { os: "windows" });
    await click(wrapper, "#wizard-next");
    await click(wrapper, "#wizard-back");
    expect(rows(wrapper)).toContain("os");
    expect(pressedTiles(wrapper, "os-seg")).toEqual(["windows"]);
  });

  it("back from the exit screen after a skip returns to the scope question", async () => {
    const { wrapper } = mount({ mark: "browser" });
    await click(wrapper, "#wizard-skip");
    await click(wrapper, "#wizard-back");
    expect(cursor(wrapper)).toBe("scope");
    expect(pressedTiles(wrapper, "scope-seg")).toEqual([]);
  });

  it("the questions page has no Back, however far it is open", () => {
    const { wrapper } = mount({ mark: "browser", scope: "system" });
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
  it("counts the open step of the all-apps path", async () => {
    const { wrapper } = mount({ mark: "browser", scope: "system" });
    const seen = [cursor(wrapper) + ":" + progress(wrapper)];
    await click(wrapper, "#wizard-next");
    seen.push(cursor(wrapper) + ":" + progress(wrapper));
    expect(seen).toEqual(["guide:Step 4 of 5", "exit:Step 5 of 5"]);
  });
});

describe("SetupWizard > focus", () => {
  it("moves to the row that opened after an answer, not on mounting", async () => {
    const { wrapper } = mount({ mark: "browser" });
    expect(document.activeElement).not.toBe(wrapper.get("#wizard-q-scope").element);
    await wrapper.get("#scope-system").trigger("click");
    await nextTick();
    await flushPromises();
    expect(document.activeElement).toBe(wrapper.get("#wizard-q-guide").element);
  });

  it("moves to the heading of the exit screen after Next", async () => {
    const { wrapper } = mount({ mark: "browser", scope: "system" });
    await wrapper.get("#wizard-next").trigger("click");
    await nextTick();
    await flushPromises();
    expect(document.activeElement).toBe(wrapper.get("#wizard-q").element);
  });
});

describe("SetupWizard > proxy off", () => {
  it("proxy off shows the settings hint and three steps", async () => {
    const { wrapper } = mount({ mark: "browser", proxyOn: false });
    expect(cursor(wrapper)).toBe("scope");
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
    expect(rows(wrapper)).toContain("os");
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
