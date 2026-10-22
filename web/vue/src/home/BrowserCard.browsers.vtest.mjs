/* "Connect the browser to Mesh Town", the "Your browser" buttons in the manual block:
 * Safari exists only on macOS, so the Safari button and the word "Safari" in the `#proxy-browser-hint` hint
 * show only when macOS is selected. The rest is in `BrowserCard.vtest.mjs`, `BrowserCard.pac.vtest.mjs`,
 * `BrowserCard.manual.vtest.mjs`. Texts come from `common/pac-texts.ts`. */

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { MANUAL_BROWSER_HINT, MANUAL_BROWSER_HINT_NO_SAFARI } from "../common/pac-texts.ts";
import { createStore, STORE_KEY } from "../core/store.ts";
import { applySnapshot } from "../testing/snapshot.mjs";
import { baseConfig, baseState } from "../testing/section.mjs";
import BrowserCard from "./BrowserCard.vue";

const PROXY_ON = { socks: { enabled: true, listen: "127.0.0.1:1080", pac_url: "http://127.0.0.1:1080/proxy.pac" } };
const MAC_CHROME = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";
const MAC_SAFARI = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15";
const LINUX_FIREFOX = "Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0";
const WINDOWS_EDGE = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0";
// The step line for Safari outside macOS comes from `browser-setup.ts` (`SAFARI_ONLY_MAC`); it must not change.
const SAFARI_ONLY_MAC = "Safari exists only on macOS. Choose another browser here — Firefox is the most reliable.";
const NO_SAFARI = ["chrome", "edge", "firefox"];
let mounted;

beforeEach(() => localStorage.clear());
afterEach(() => {
  mounted.wrapper.unmount();
  Reflect.deleteProperty(navigator, "userAgent");
});

describe("BrowserCard > browser buttons by the selected system", () => {
  describe("macOS", () => {
    it("four buttons in the order Chrome, Edge, Safari, Firefox, #browser-safari exists", async () => {
      const { wrapper } = await mountExpanded(MAC_CHROME);
      expect(wrapper.find("#browser-safari").exists(), "the Safari button on macOS").toBe(true);
      expect(browserIds(wrapper)).toEqual(["chrome", "edge", "safari", "firefox"]);
      expect(browserLabels(wrapper)).toEqual(["Chrome", "Edge", "Safari", "Firefox"]);
    });

    it("the hint above the buttons is the old one, about Chrome, Edge and Safari", async () => {
      const { wrapper } = await mountExpanded(MAC_CHROME);
      expect(wrapper.get("#proxy-browser-hint").text()).toBe(MANUAL_BROWSER_HINT);
      expect(wrapper.get("#proxy-browser-hint").text()).toContain("Safari");
    });

    it("Safari in userAgent: the Safari button is pressed, the others are not", async () => {
      const { wrapper } = await mountExpanded(MAC_SAFARI);
      expect(pressedIds(wrapper)).toEqual(["safari"]);
    });
  });

  describe.each([["Linux", LINUX_FIREFOX], ["Windows", WINDOWS_EDGE]])("%s", (_name, ua) => {
    it("buttons Chrome, Edge, Firefox in this order, no #browser-safari", async () => {
      const { wrapper } = await mountExpanded(ua);
      expect(wrapper.find("#browser-safari").exists(), "no Safari button outside macOS").toBe(false);
      expect(browserIds(wrapper)).toEqual(NO_SAFARI);
      expect(browserLabels(wrapper)).toEqual(["Chrome", "Edge", "Firefox"]);
    });

    it("the hint above the buttons has no Safari, it is about Chrome and Edge", async () => {
      const { wrapper } = await mountExpanded(ua);
      expect(wrapper.get("#proxy-browser-hint").text()).toBe(MANUAL_BROWSER_HINT_NO_SAFARI);
      expect(wrapper.get("#proxy-browser-hint").text()).not.toContain("Safari");
    });

    it("the word \"Safari\" is nowhere in the manual block: not in the intro, not in the hint, not in the steps", async () => {
      const { wrapper } = await mountExpanded(ua);
      expect(wrapper.get("#proxy-setup").text()).not.toContain("Safari");
    });

    it("one button is pressed — the browser from userAgent", async () => {
      const { wrapper } = await mountExpanded(ua);
      expect(pressedIds(wrapper)).toHaveLength(1);
    });
  });

  describe("system change with the block expanded", () => {
    it("macOS → Linux → Windows → macOS: Safari disappears and returns together with the hint", async () => {
      const { wrapper } = await mountExpanded(MAC_CHROME);
      await wrapper.get("#os-linux").trigger("click");
      expect(browserIds(wrapper)).toEqual(NO_SAFARI);
      expect(wrapper.get("#proxy-browser-hint").text()).toBe(MANUAL_BROWSER_HINT_NO_SAFARI);
      await wrapper.get("#os-windows").trigger("click");
      expect(browserIds(wrapper)).toEqual(NO_SAFARI);
      expect(wrapper.get("#proxy-browser-hint").text()).toBe(MANUAL_BROWSER_HINT_NO_SAFARI);
      await wrapper.get("#os-mac").trigger("click");
      expect(browserIds(wrapper)).toEqual(["chrome", "edge", "safari", "firefox"]);
      expect(wrapper.get("#proxy-browser-hint").text()).toBe(MANUAL_BROWSER_HINT);
    });

    it("Linux → macOS: Safari appears, the hint mentions \"Safari\"", async () => {
      const { wrapper } = await mountExpanded(LINUX_FIREFOX);
      expect(wrapper.find("#browser-safari").exists()).toBe(false);
      await wrapper.get("#os-mac").trigger("click");
      expect(wrapper.find("#browser-safari").exists()).toBe(true);
      expect(wrapper.get("#proxy-browser-hint").text()).toBe(MANUAL_BROWSER_HINT);
    });
  });
});

describe("BrowserCard > Safari was selected, the system changed to Linux or Windows", () => {
  describe.each([["Linux", "#os-linux"], ["Windows", "#os-windows"]])("%s", (_name, osButton) => {
    it("no Safari button, Chrome, Edge and Firefox are visible, none is pressed (aria-pressed=false on all)", async () => {
      const { wrapper } = await mountWithSafariPicked();
      await wrapper.get(osButton).trigger("click");
      expect(wrapper.find("#browser-safari").exists()).toBe(false);
      expect(browserIds(wrapper)).toEqual(NO_SAFARI);
      for (const id of NO_SAFARI) {
        expect(wrapper.get("#browser-" + id).attributes("aria-pressed"), "aria-pressed of " + id).toBe("false");
      }
      expect(pressedIds(wrapper)).toEqual([]);
    });

    it("the manual block steps are the one old line \"Safari exists only on macOS…\", no warning", async () => {
      const { wrapper } = await mountWithSafariPicked();
      await wrapper.get(osButton).trigger("click");
      expect(stepTexts(wrapper)).toEqual([SAFARI_ONLY_MAC]);
      expect(wrapper.get("#proxy-warn").text()).toBe("");
    });

    it("chose Firefox — the button is pressed, the Safari line is replaced by the Firefox steps", async () => {
      const { wrapper } = await mountWithSafariPicked();
      await wrapper.get(osButton).trigger("click");
      await wrapper.get("#browser-firefox").trigger("click");
      expect(pressedIds(wrapper)).toEqual(["firefox"]);
      expect(stepTexts(wrapper)).not.toContain(SAFARI_ONLY_MAC);
      expect(stepTexts(wrapper)[0]).toContain("Firefox");
    });
  });

  it("back on macOS — Safari is in place again and pressed, the Safari steps for macOS", async () => {
    const { wrapper } = await mountWithSafariPicked();
    await wrapper.get("#os-linux").trigger("click");
    await wrapper.get("#os-mac").trigger("click");
    expect(pressedIds(wrapper)).toEqual(["safari"]);
    expect(stepTexts(wrapper)).not.toContain(SAFARI_ONLY_MAC);
  });

  it("the hint above the buttons after the change — without Safari", async () => {
    const { wrapper } = await mountWithSafariPicked();
    await wrapper.get("#os-linux").trigger("click");
    expect(wrapper.get("#proxy-browser-hint").text()).toBe(MANUAL_BROWSER_HINT_NO_SAFARI);
  });

  it("Safari was detected from userAgent (not by a click), the system changed to Linux — the same: none pressed, one line about macOS", async () => {
    const { wrapper } = await mountExpanded(MAC_SAFARI);
    expect(pressedIds(wrapper)).toEqual(["safari"]);
    await wrapper.get("#os-linux").trigger("click");
    expect(pressedIds(wrapper)).toEqual([]);
    expect(stepTexts(wrapper)).toEqual([SAFARI_ONLY_MAC]);
  });
});

// ids of the browser buttons in DOM order (data-browser)
function browserIds(wrapper) {
  return wrapper.get("#browser-seg").findAll("button").map((b) => b.attributes("data-browser"));
}

function browserLabels(wrapper) {
  return wrapper.get("#browser-seg").findAll("button").map((b) => b.text());
}

function pressedIds(wrapper) {
  return wrapper.get("#browser-seg").findAll("button")
    .filter((b) => b.attributes("aria-pressed") === "true")
    .map((b) => b.attributes("data-browser"));
}

function stepTexts(wrapper) {
  return wrapper.get("#proxy-steps").findAll("li").map((li) => li.text());
}

// macOS, the manual block expanded, the Safari button pressed
async function mountWithSafariPicked() {
  const card = await mountExpanded(MAC_CHROME);
  await card.wrapper.get("#browser-safari").trigger("click");
  expect(pressedIds(card.wrapper), "on macOS Safari is selected").toEqual(["safari"]);
  return card;
}

async function mountExpanded(userAgent) {
  const card = mountWith(userAgent);
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
