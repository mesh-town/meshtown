/* "Connect the browser to Mesh Town", the auto-config (PAC) part: the starting system from `userAgent`,
 * the steps of every system (computers, Android, iOS), a click on the system, copying the address and the note
 * about a clipboard refusal (separate for the PAC address and the manual block). The rest of the card is in `BrowserCard.vtest.mjs`. */

import { nextTick } from "vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import {
  ANDROID_INTRO, ANDROID_STEPS, IOS_NO_APP, LINUX_SMALL, LINUX_STEPS, MAC_SMALL, MAC_STEPS, OS_LABEL, PAC_COPY_FALLBACK, WINDOWS_SMALL, WINDOWS_STEPS,
} from "../common/pac-texts.ts";
import { createStore, STORE_KEY } from "../core/store.ts";
import { applySnapshot } from "../testing/snapshot.mjs";
import { baseConfig, baseState } from "../testing/section.mjs";
import BrowserCard from "./BrowserCard.vue";

const PAC_URL = "http://127.0.0.1:1080/proxy.pac";
const PROXY_ON = { socks: { enabled: true, listen: "127.0.0.1:1080", pac_url: PAC_URL } };
const OS_IDS = ["mac", "windows", "linux", "android", "ios"];
const MAC_CHROME = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";
const LINUX_FIREFOX = "Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0";
const WINDOWS_EDGE = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0";
const IPHONE_SAFARI = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1";
const ANDROID_CHROME = "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Mobile Safari/537.36";
const PORT_NOTE = "Clipboard unavailable — select the port in the step and copy it manually.";
const HOST_NOTE = "Clipboard unavailable — select the address in the step and copy it manually.";
let mounted;

beforeEach(() => {
  localStorage.clear();
  localStorage.setItem("meshtown.proxyScope", "system"); // the scope question is answered; its own tests are in BrowserCard.scope.vtest.mjs
});
afterEach(() => {
  mounted.wrapper.unmount();
  Reflect.deleteProperty(navigator, "userAgent");
  Reflect.deleteProperty(navigator, "clipboard");
  vi.useRealTimers();
});

describe("BrowserCard > system and auto-config steps", () => {
  describe("the starting system is detected from userAgent", () => {
    it.each([
      ["macOS, Chrome", MAC_CHROME, "mac"],
      ["Linux, Firefox", LINUX_FIREFOX, "linux"],
      ["Windows, Edge", WINDOWS_EDGE, "windows"],
      ["iPhone, Safari (the string has \"Mac OS X\")", IPHONE_SAFARI, "ios"],
      ["Android, Chrome (the string has \"Linux\")", ANDROID_CHROME, "android"],
    ])("%s: one button is pressed — the right system", (_name, ua, id) => {
      expectPressed(mountWith(ua).wrapper, id);
    });

    it("the \"Your system\" group with a label and five buttons in the order macOS, Windows, Linux, Android, iOS", () => {
      const { wrapper } = mountWith(MAC_CHROME);
      const group = wrapper.get("#os-seg");
      expect(group.attributes("role")).toBe("group");
      expect(group.attributes("aria-label")).toBe("Your system");
      expect(wrapper.get("#os-label").text()).toBe(OS_LABEL);
      expect(group.findAll("button").map((b) => b.text())).toEqual(["macOS", "Windows", "Linux", "Android", "iOS"]);
    });
  });

  describe("macOS", () => {
    it("five numbered steps verbatim, the small lines as separate paragraphs, no stub", () => {
      const { wrapper } = mountWith(MAC_CHROME);
      expect(wrapper.get("#pac-steps").element.tagName).toBe("OL");
      expect(stepTexts(wrapper)).toEqual(MAC_STEPS);
      expect(smallTexts(wrapper)).toEqual(MAC_SMALL);
      expect(wrapper.get("#pac-os-note").isVisible()).toBe(false);
    });
  });

  describe("Linux", () => {
    it("three GNOME steps verbatim and two small lines (GNOME 43, KDE), no stub", () => {
      const { wrapper } = mountWith(LINUX_FIREFOX);
      expect(stepTexts(wrapper)).toEqual(LINUX_STEPS);
      expect(smallTexts(wrapper)).toEqual(LINUX_SMALL);
      expect(wrapper.get("#pac-os-note").isVisible()).toBe(false);
    });
  });

  describe("Windows", () => {
    it("its steps verbatim and the Windows 10 small line, no note", () => {
      const { wrapper } = mountWith(WINDOWS_EDGE);
      expect(stepTexts(wrapper)).toEqual(WINDOWS_STEPS);
      expect(smallTexts(wrapper)).toEqual(WINDOWS_SMALL);
      expect(wrapper.get("#pac-os-note").isVisible()).toBe(false);
    });
  });

  describe("Android", () => {
    it("the intro as a visible note, the app steps in the list, no small lines", () => {
      const { wrapper } = mountWith(ANDROID_CHROME);
      expect(wrapper.get("#pac-os-note").isVisible()).toBe(true);
      expect(wrapper.get("#pac-os-note").text()).toBe(ANDROID_INTRO);
      expect(stepTexts(wrapper)).toEqual(ANDROID_STEPS);
      expect(smallTexts(wrapper)).toEqual([]);
    });
  });

  describe("iOS", () => {
    it("only the line that there is no app: no step list, no small lines", () => {
      const { wrapper } = mountWith(IPHONE_SAFARI);
      expect(wrapper.find("#pac-steps").exists(), "an empty <ol> is not rendered").toBe(false);
      expect(smallTexts(wrapper)).toEqual([]);
      expect(wrapper.get("#pac-os-note").isVisible()).toBe(true);
      expect(wrapper.get("#pac-os-note").text()).toBe(IOS_NO_APP);
    });
  });

  describe("click on another system", () => {
    it("macOS → Windows: Windows is pressed, the Windows steps and small line replace the macOS ones", async () => {
      const { wrapper } = mountWith(MAC_CHROME);
      await wrapper.get("#os-windows").trigger("click");
      expectPressed(wrapper, "windows");
      expect(stepTexts(wrapper)).toEqual(WINDOWS_STEPS);
      expect(smallTexts(wrapper)).toEqual(WINDOWS_SMALL);
      expect(wrapper.get("#pac-os-note").isVisible()).toBe(false);
    });

    it("Windows → Linux: the Linux steps and its small lines, the stub is hidden", async () => {
      const { wrapper } = mountWith(WINDOWS_EDGE);
      await wrapper.get("#os-linux").trigger("click");
      expectPressed(wrapper, "linux");
      expect(stepTexts(wrapper)).toEqual(LINUX_STEPS);
      expect(smallTexts(wrapper)).toEqual(LINUX_SMALL);
      expect(wrapper.get("#pac-os-note").isVisible()).toBe(false);
    });

    it("Linux → iOS → Android → macOS: the iOS line, the Android intro with steps, then the macOS steps", async () => {
      const { wrapper } = mountWith(LINUX_FIREFOX);
      await wrapper.get("#os-ios").trigger("click");
      expectPressed(wrapper, "ios");
      expect(wrapper.get("#pac-os-note").text()).toBe(IOS_NO_APP);
      await wrapper.get("#os-android").trigger("click");
      expectPressed(wrapper, "android");
      expect(wrapper.get("#pac-os-note").text()).toBe(ANDROID_INTRO);
      expect(stepTexts(wrapper)).toEqual(ANDROID_STEPS);
      await wrapper.get("#os-mac").trigger("click");
      expectPressed(wrapper, "mac");
      expect(stepTexts(wrapper)).toEqual(MAC_STEPS);
    });

    it("choosing a system doesn't hide the PAC address and doesn't expand the manual block", async () => {
      const { wrapper } = mountWith(MAC_CHROME);
      await wrapper.get("#os-linux").trigger("click");
      expect(wrapper.get("#pac-url-text").text()).toBe(PAC_URL);
      expect(wrapper.find("#proxy-setup").exists()).toBe(false);
    });
  });
});

describe("BrowserCard > copying the PAC address", () => {
  beforeEach(() => vi.useFakeTimers());

  it("\"Copy address\" puts the PAC address on the clipboard, the button says \"Copied\" for 2 s", async () => {
    const writes = trapClipboard();
    const { wrapper } = mountWith(MAC_CHROME);
    await wrapper.get("#pac-copy-btn").trigger("click");
    await flushPromises();
    expect(writes).toEqual([PAC_URL]);
    expect(wrapper.get("#pac-copy-btn").text()).toBe("Copied");
    await vi.advanceTimersByTimeAsync(1999);
    expect(wrapper.get("#pac-copy-btn").text(), "two seconds have not passed yet").toBe("Copied");
    await vi.advanceTimersByTimeAsync(1);
    await nextTick();
    expect(wrapper.get("#pac-copy-btn").text()).toBe("Copy address");
  });

  it("before copying the clipboard refusal note is hidden", () => {
    expect(mountWith(MAC_CHROME).wrapper.get("#pac-copy-note").isVisible()).toBe(false);
  });

  it("no clipboard: PAC_COPY_FALLBACK note under the address, the button unchanged, the manual block notes don't show it", async () => {
    noClipboard();
    const { wrapper } = mountWith(MAC_CHROME);
    await wrapper.get("#manual-toggle").trigger("click");
    await wrapper.get("#pac-copy-btn").trigger("click");
    await flushPromises();
    const note = wrapper.get("#pac-copy-note");
    expect(note.isVisible()).toBe(true);
    expect(note.text()).toBe(PAC_COPY_FALLBACK);
    expect(wrapper.get("#pac-copy-btn").text()).toBe("Copy address");
    expect(wrapper.get("#proxy-copy-note").isVisible()).toBe(false);
  });

  it("after a refusal a successful copy removes the note", async () => {
    noClipboard();
    const { wrapper } = mountWith(MAC_CHROME);
    await wrapper.get("#pac-copy-btn").trigger("click");
    await flushPromises();
    expect(wrapper.get("#pac-copy-note").isVisible()).toBe(true);
    trapClipboard();
    await wrapper.get("#pac-copy-btn").trigger("click");
    await flushPromises();
    expect(wrapper.get("#pac-copy-note").isVisible()).toBe(false);
  });
});

describe("BrowserCard > copying in the manual block", () => {
  beforeEach(() => vi.useFakeTimers());

  it("\"Copy port\" puts the port on the clipboard, \"Copied\" only on it — the PAC address button's label is unchanged", async () => {
    const writes = trapClipboard();
    const { wrapper } = await mountExpanded();
    await wrapper.get("#socks-copy-port-btn").trigger("click");
    await flushPromises();
    expect(writes).toEqual(["1080"]);
    expect(wrapper.get("#socks-copy-port-btn").text()).toBe("Copied");
    expect(wrapper.get("#pac-copy-btn").text()).toBe("Copy address");
  });

  it("\"Copy address\" in the manual block puts the host, not the PAC address", async () => {
    const writes = trapClipboard();
    const { wrapper } = await mountExpanded();
    await wrapper.get("#socks-copy-btn").trigger("click");
    await flushPromises();
    expect(writes).toEqual(["127.0.0.1"]);
  });

  it("no clipboard, the port is copied: the port note is in #proxy-copy-note, the PAC note is hidden", async () => {
    noClipboard();
    const { wrapper } = await mountExpanded();
    await wrapper.get("#socks-copy-port-btn").trigger("click");
    await flushPromises();
    expect(wrapper.get("#proxy-copy-note").isVisible()).toBe(true);
    expect(wrapper.get("#proxy-copy-note").text()).toBe(PORT_NOTE);
    expect(wrapper.get("#pac-copy-note").isVisible()).toBe(false);
  });

  it("no clipboard, the address in the step is copied: the address note is in #proxy-copy-note, the PAC note is hidden", async () => {
    noClipboard();
    const { wrapper } = await mountExpanded();
    await wrapper.get("#socks-copy-btn").trigger("click");
    await flushPromises();
    expect(wrapper.get("#proxy-copy-note").text()).toBe(HOST_NOTE);
    expect(wrapper.get("#proxy-copy-note").isVisible()).toBe(true);
    expect(wrapper.get("#pac-copy-note").isVisible()).toBe(false);
  });
});

const stepTexts = (wrapper) => wrapper.findAll("#pac-steps li").map((li) => li.text());
const smallTexts = (wrapper) => wrapper.findAll(".pac-small").map((p) => p.text());

function expectPressed(wrapper, id) {
  const states = Object.fromEntries(OS_IDS.map((os) => [os, wrapper.get("#os-" + os).attributes("aria-pressed")]));
  expect(states).toEqual(Object.fromEntries(OS_IDS.map((os) => [os, String(os === id)])));
}

function trapClipboard() {
  const writes = [];
  Object.defineProperty(navigator, "clipboard", { value: { writeText: async (t) => { writes.push(t); } }, configurable: true });
  return writes;
}

const noClipboard = () => Object.defineProperty(navigator, "clipboard", { value: undefined, configurable: true });

async function mountExpanded() {
  const card = mountWith(MAC_CHROME);
  await card.wrapper.get("#manual-toggle").trigger("click");
  return card;
}

function mountWith(userAgent, state = baseState(PROXY_ON)) {
  Object.defineProperty(navigator, "userAgent", { value: userAgent, configurable: true });
  const store = createStore();
  applySnapshot(store, { state, config: baseConfig(), configRev: 1 });
  const wrapper = mount(BrowserCard, { attachTo: document.body, global: { provide: { [STORE_KEY]: store } } });
  return (mounted = { wrapper, store });
}
