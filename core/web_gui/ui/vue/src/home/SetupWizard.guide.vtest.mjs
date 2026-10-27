/* The guide screen of the setup wizard: the numbered steps for the chosen system and browser, the address with its Copy
 * button, the phone variants, the "one browser" explanation, the collapsed extras and the router's networks summary. */

import { afterEach, describe, expect, it } from "vitest";
import {
  ANDROID_INTRO, ANDROID_STEPS, FIREFOX_STEPS, IOS_NO_APP, LINUX_SMALL, LINUX_STEPS, MAC_SMALL, MAC_STEPS, PAC_COPY_FALLBACK, WINDOWS_SMALL, WINDOWS_STEPS,
} from "../common/pac-texts.ts";
import { laptopState, routerState, netsConfig } from "../testing/nets-states.mjs";
import {
  ANDROID_CHROME, IPHONE_SAFARI, PAC_URL, SCOPE_KEY, click, mountWizard, plain, progress, step, toGuide, unmountWizard,
} from "../testing/wizard.mjs";
import { UPDATE_ANDROID_DOWNLOAD_URL } from "./update-texts.ts";
import { TERM_TIPS } from "./wizard/term-hints.ts";
import { WIZ_ONE_BROWSER, WIZ_ONE_BROWSER_OTHER } from "./wizard/wizard-texts.ts";

let mounted;
const mount = (opts) => (mounted = mountWizard({ mark: "browser", scope: "system", ...opts }));
const items = (wrapper, selector) => wrapper.findAll(selector).map(plain);
const smallLines = (wrapper) => items(wrapper, ".pac-small");

afterEach(() => unmountWizard(mounted));

describe("SetupWizard > guide > all apps", () => {
  it.each([
    ["mac", MAC_STEPS, MAC_SMALL],
    ["windows", WINDOWS_STEPS, WINDOWS_SMALL],
    ["linux", LINUX_STEPS, LINUX_SMALL],
  ])("system steps per OS: %s", async (os, steps, small) => {
    const { wrapper } = mount();
    await toGuide(wrapper, { os });
    expect(items(wrapper, "#pac-steps > li").slice(1)).toEqual(steps);
    expect(smallLines(wrapper).slice(0, small.length)).toEqual(small);
    expect(wrapper.get("#pac-steps > li").find("#pac-copy-btn").exists(), "the first item is the address").toBe(true);
  });

  it("the first item holds the address and a Copy button", async () => {
    const { wrapper } = mount();
    await toGuide(wrapper);
    const first = wrapper.get("#pac-steps > li");
    expect(first.get("#pac-url-text").text()).toBe(PAC_URL);
    expect(plain(first)).toContain("Copy this address (PAC):");
    expect(first.get("#pac-copy-btn").text()).toBe("Copy address");
  });

  it("the guide is titled by the browser and shows its steps after the system ones", async () => {
    const { wrapper } = mount();
    await toGuide(wrapper, { browser: "edge" });
    expect(wrapper.get("#wizard-q").text()).toBe("Set up Edge");
    expect(wrapper.get("#guide-browser-title").text()).toBe("Edge");
    expect(items(wrapper, "#browser-steps > li")).toHaveLength(2);
    expect(step(wrapper)).toBe("guide");
  });

  it("the Chrome line about GNOME and KDE shows on Linux only", async () => {
    const linux = mount();
    await toGuide(linux.wrapper, { os: "linux", browser: "chrome" });
    expect(smallLines(linux.wrapper).some((t) => t.startsWith("On Linux, Chrome follows"))).toBe(true);
    unmountWizard(linux);
    const { wrapper } = mount();
    await toGuide(wrapper, { os: "mac", browser: "chrome" });
    expect(smallLines(wrapper).some((t) => t.startsWith("On Linux, Chrome follows"))).toBe(false);
  });

  it("the word PAC is a focusable term with its tip", async () => {
    const { wrapper } = mount();
    await toGuide(wrapper);
    const term = wrapper.get("#pac-steps > li .term");
    expect(term.text()).toContain("PAC");
    expect(term.attributes("tabindex")).toBe("0");
    expect(term.get('[role="tooltip"]').text()).toBe(TERM_TIPS.PAC);
  });
});

describe("SetupWizard > guide > copying the address", () => {
  it("copy button copies the setup address", async () => {
    const writes = trapClipboard();
    const { wrapper } = mount();
    await toGuide(wrapper);
    await click(wrapper, "#pac-copy-btn");
    expect(writes).toEqual([PAC_URL]);
    expect(wrapper.get("#pac-copy-btn").text()).toBe("Copied");
  });

  it("no clipboard: the note under the address appears and the button stays", async () => {
    Object.defineProperty(navigator, "clipboard", { value: undefined, configurable: true });
    const { wrapper } = mount();
    await toGuide(wrapper);
    expect(wrapper.get("#pac-copy-note").isVisible(), "hidden before a refusal").toBe(false);
    await click(wrapper, "#pac-copy-btn");
    expect(wrapper.get("#pac-copy-note").isVisible()).toBe(true);
    expect(wrapper.get("#pac-copy-note").text()).toBe(PAC_COPY_FALLBACK);
    expect(wrapper.get("#pac-copy-btn").text()).toBe("Copy address");
  });
});

describe("SetupWizard > guide > only in one browser", () => {
  it("firefox has its own steps and the copy row", async () => {
    const { wrapper } = mount({ scope: "browser" });
    await toGuide(wrapper, { browser: "firefox" });
    expect(wrapper.get("#pac-url-text").text()).toBe(PAC_URL);
    expect(items(wrapper, "#pac-steps > li").slice(1)).toEqual(FIREFOX_STEPS);
    expect(wrapper.find("#guide-to-system").exists()).toBe(false);
    expect(wrapper.find("#more-options").exists()).toBe(true);
  });

  it.each([["chrome", "Chrome"], ["edge", "Edge"], ["safari", "Safari"]])("%s explains it uses the system setting and offers all apps", async (id, name) => {
    const { wrapper } = mount({ scope: "browser" });
    await toGuide(wrapper, { browser: id });
    expect(wrapper.get("#guide-intro").text()).toBe(WIZ_ONE_BROWSER(name));
    expectOnlyTheSwitch(wrapper);
  });

  it("other explains it uses the system setting and offers all apps", async () => {
    const { wrapper } = mount({ scope: "browser" });
    await toGuide(wrapper, { browser: "other" });
    expect(wrapper.get("#guide-intro").text()).toBe(WIZ_ONE_BROWSER_OTHER);
    expectOnlyTheSwitch(wrapper);
  });

  it("switch to all apps returns to the system question", async () => {
    const { wrapper } = mount({ scope: "browser" });
    await toGuide(wrapper, { browser: "chrome" });
    await click(wrapper, "#guide-to-system");
    expect(step(wrapper)).toBe("os");
    expect(progress(wrapper)).toBe("Step 3 of 6");
    expect(localStorage.getItem(SCOPE_KEY)).toBe("system");
  });
});

describe("SetupWizard > guide > phones", () => {
  it("android shows the app link and no copy row", async () => {
    const { wrapper } = mount({ ua: ANDROID_CHROME });
    await toGuide(wrapper);
    const link = wrapper.get("#guide-app-link");
    expect(link.attributes("href")).toBe(UPDATE_ANDROID_DOWNLOAD_URL);
    expect(link.attributes("target")).toBe("_blank");
    expect(link.attributes("rel")).toContain("noopener");
    expect(wrapper.get("#guide-intro").text()).toBe(ANDROID_INTRO);
    expect(items(wrapper, "#pac-steps > li").slice(1)).toEqual(ANDROID_STEPS);
    expect(wrapper.get("#pac-steps > li").text()).toContain("On the phone, download the Mesh Town app:");
    expect(wrapper.get("#pac-steps > li").text()).toContain("app: meshtown.apk");
    expect(wrapper.find("#pac-copy-btn").exists()).toBe(false);
    expect(wrapper.find("#pac-url-text").exists()).toBe(false);
    expect(wrapper.find("#more-options").exists()).toBe(false);
  });

  it("ios shows only the no-app line", async () => {
    const { wrapper } = mount({ ua: IPHONE_SAFARI });
    await toGuide(wrapper);
    expect(wrapper.get("#guide-intro").text()).toBe(IOS_NO_APP);
    expect(wrapper.find("#pac-steps").exists()).toBe(false);
    expect(wrapper.find("#guide-app-link").exists()).toBe(false);
    expect(wrapper.find("#pac-copy-btn").exists()).toBe(false);
    expect(wrapper.find("#more-options").exists()).toBe(false);
  });
});

describe("SetupWizard > guide > extras", () => {
  it("more options are collapsed and hold strict choice and manual setup", async () => {
    const { wrapper } = mount();
    await toGuide(wrapper);
    const more = wrapper.get("#more-options");
    expect(more.element.open).toBe(false);
    expect(more.find("#strict-choice").exists()).toBe(true);
    expect(more.find("#manual-toggle").exists()).toBe(true);
    expect(more.element.compareDocumentPosition(wrapper.get("#browser-steps").element) & Node.DOCUMENT_POSITION_PRECEDING, "after the browser steps").toBeTruthy();
  });

  it("the manual block opens without a browser switcher", async () => {
    const { wrapper } = mount();
    await toGuide(wrapper);
    await click(wrapper, "#manual-toggle");
    expect(wrapper.find("#proxy-setup").exists()).toBe(true);
    expect(wrapper.find("#browser-seg").exists(), "the browser comes from the wizard").toBe(false);
  });
});

describe("SetupWizard > guide > networks", () => {
  it("a router shows the networks summary under the first step", async () => {
    const state = routerState();
    const { wrapper } = mount({ state, config: netsConfig(state) });
    await toGuide(wrapper);
    expect(wrapper.get("#pac-steps > li").find("#nets-summary").exists()).toBe(true);
  });

  it("a laptop has no networks summary on this screen", async () => {
    const state = laptopState();
    const { wrapper } = mount({ state, config: netsConfig(state) });
    await toGuide(wrapper);
    expect(wrapper.find("#nets-summary").exists()).toBe(false);
  });
});

function expectOnlyTheSwitch(wrapper) {
  expect(wrapper.find("#guide-to-system").text()).toBe("Set up for all apps");
  expect(wrapper.find("#pac-copy-btn").exists()).toBe(false);
  expect(wrapper.find("#more-options").exists()).toBe(false);
}

function trapClipboard() {
  const writes = [];
  Object.defineProperty(navigator, "clipboard", { value: { writeText: async (t) => { writes.push(t); } }, configurable: true });
  return writes;
}
