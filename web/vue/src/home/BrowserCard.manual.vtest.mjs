/* "Connect the browser to Mesh Town", the card tails: the manual block depends on
 * the selected system (it is absent for "Phone", macOS, Linux and Windows each have their own intro) and the two
 * "Copy address" buttons are distinguishable for screen readers (`aria-label`). The rest is in
 * `BrowserCard.vtest.mjs` and `BrowserCard.pac.vtest.mjs`. Texts come from `common/pac-texts.ts`. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import {
  HOST_COPY_ARIA, MANUAL_INTRO_BASE, MANUAL_TAIL_LINUX, MANUAL_TAIL_MAC, MANUAL_TAIL_WINDOWS, MANUAL_TOGGLE, MANUAL_TOGGLE_OPEN, PAC_COPY_ARIA, PHONE_SOON,
} from "../common/pac-texts.ts";
import { createStore, STORE_KEY } from "../core/store.ts";
import { applySnapshot } from "../testing/snapshot.mjs";
import { baseConfig, baseState } from "../testing/section.mjs";
import BrowserCard from "./BrowserCard.vue";

const LISTEN = "127.0.0.1:1080";
const PAC_URL = "http://127.0.0.1:1080/proxy.pac";
const PROXY_ON = { socks: { enabled: true, listen: LISTEN, pac_url: PAC_URL } };
const MAC_CHROME = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";
const LINUX_FIREFOX = "Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0";
const WINDOWS_EDGE = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0";
const IPHONE_SAFARI = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1";
const ANDROID_CHROME = "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Mobile Safari/537.36";
const INTRO = {
  mac: MANUAL_INTRO_BASE + " " + MANUAL_TAIL_MAC,
  linux: MANUAL_INTRO_BASE + " " + MANUAL_TAIL_LINUX,
  windows: MANUAL_INTRO_BASE + " " + MANUAL_TAIL_WINDOWS,
};
let mounted;

beforeEach(() => localStorage.clear());
afterEach(() => {
  mounted.wrapper.unmount();
  Reflect.deleteProperty(navigator, "userAgent");
  Reflect.deleteProperty(navigator, "clipboard");
  vi.useRealTimers();
});

describe("BrowserCard > manual block and the selected system", () => {
  describe("phone", () => {
    it.each([
      ["iPhone", IPHONE_SAFARI],
      ["Android", ANDROID_CHROME],
    ])("%s: no \"Not working? Set up manually\" link and no manual block, the PAC address and the \"coming soon\" note are in place", (_name, ua) => {
      const { wrapper } = mountWith(ua);
      expect(wrapper.find("#manual-toggle").exists(), "the \"Set up manually\" link").toBe(false);
      expect(wrapper.find("#proxy-setup").exists(), "the manual block").toBe(false);
      expect(wrapper.get("#pac-url-text").text()).toBe(PAC_URL);
      expect(wrapper.get("#pac-os-note").text()).toBe(PHONE_SOON);
    });

    it("expanded the manual block on macOS, switched to \"Phone\": no link, no block", async () => {
      const { wrapper } = await mountExpanded(MAC_CHROME);
      expect(wrapper.find("#proxy-setup").exists(), "on macOS the block is expanded").toBe(true);
      await wrapper.get("#os-phone").trigger("click");
      expect(wrapper.find("#manual-toggle").exists()).toBe(false);
      expect(wrapper.find("#proxy-setup").exists()).toBe(false);
      expect(wrapper.find("#proxy-manual-intro").exists()).toBe(false);
    });

    it("back from \"Phone\" to macOS: the block is expanded again, the link with a ▾ arrow and aria-expanded=true", async () => {
      const { wrapper } = await mountExpanded(MAC_CHROME);
      await wrapper.get("#os-phone").trigger("click");
      await wrapper.get("#os-mac").trigger("click");
      const toggle = wrapper.get("#manual-toggle");
      expect(toggle.attributes("aria-expanded")).toBe("true");
      expect(toggle.text()).toBe(MANUAL_TOGGLE_OPEN);
      expect(wrapper.find("#proxy-setup").exists()).toBe(true);
      expect(wrapper.get("#proxy-manual-intro").text()).toBe(INTRO.mac);
    });

    it("the block was collapsed: after \"Phone\" and back to Linux it stays collapsed", async () => {
      const { wrapper } = mountWith(LINUX_FIREFOX);
      await wrapper.get("#os-phone").trigger("click");
      await wrapper.get("#os-linux").trigger("click");
      expect(wrapper.get("#manual-toggle").text()).toBe(MANUAL_TOGGLE);
      expect(wrapper.get("#manual-toggle").attributes("aria-expanded")).toBe("false");
      expect(wrapper.find("#proxy-setup").exists()).toBe(false);
    });
  });

  describe("desktop systems", () => {
    it.each([["macOS", MAC_CHROME], ["Linux", LINUX_FIREFOX], ["Windows", WINDOWS_EDGE]])("%s: the link is in place", (_name, ua) => {
      expect(mountWith(ua).wrapper.get("#manual-toggle").text()).toBe(MANUAL_TOGGLE);
    });
  });
});

describe("BrowserCard > manual block intro by system", () => {
  it.each([
    ["macOS", MAC_CHROME, INTRO.mac],
    ["Linux", LINUX_FIREFOX, INTRO.linux],
    ["Windows", WINDOWS_EDGE, INTRO.windows],
  ])("%s: the common part and the tail of this system", async (_name, ua, intro) => {
    const { wrapper } = await mountExpanded(ua);
    expect(wrapper.get("#proxy-setup").get("#proxy-manual-intro").text()).toBe(intro);
  });

  it("Windows: no word \"above\" — there are no Windows steps above the manual block", async () => {
    const { wrapper } = await mountExpanded(WINDOWS_EDGE);
    expect(wrapper.get("#proxy-manual-intro").text()).not.toContain("above");
  });

  it("a system change with the block expanded changes the intro: macOS → Linux → Windows", async () => {
    const { wrapper } = await mountExpanded(MAC_CHROME);
    await wrapper.get("#os-linux").trigger("click");
    expect(wrapper.get("#proxy-manual-intro").text()).toBe(INTRO.linux);
    await wrapper.get("#os-windows").trigger("click");
    expect(wrapper.get("#proxy-manual-intro").text()).toBe(INTRO.windows);
  });

  it("one intro per card: the common part appears exactly once", async () => {
    const { wrapper } = await mountExpanded(LINUX_FIREFOX);
    expect(wrapper.findAll("#proxy-manual-intro")).toHaveLength(1);
    expect(wrapper.text().split(MANUAL_INTRO_BASE)).toHaveLength(2);
  });
});

describe("BrowserCard > labels of the copy buttons for screen readers", () => {
  beforeEach(() => vi.useFakeTimers());

  it("before copying the PAC address and proxy address buttons have different aria-labels, the visible text of both is \"Copy address\"", async () => {
    const { wrapper } = await mountExpanded(MAC_CHROME);
    expect(ariaOf(wrapper, "#pac-copy-btn")).toBe(PAC_COPY_ARIA);
    expect(ariaOf(wrapper, "#socks-copy-btn")).toBe(HOST_COPY_ARIA);
    expect(ariaOf(wrapper, "#pac-copy-btn")).not.toBe(ariaOf(wrapper, "#socks-copy-btn"));
    expect(wrapper.get("#pac-copy-btn").text()).toBe("Copy address");
    expect(wrapper.get("#socks-copy-btn").text()).toBe("Copy address");
  });

  it("after a click on the PAC address its label is \"Copied\", the host button's is unchanged", async () => {
    trapClipboard();
    const { wrapper } = await mountExpanded(MAC_CHROME);
    await wrapper.get("#pac-copy-btn").trigger("click");
    await flushPromises();
    expect(ariaOf(wrapper, "#pac-copy-btn")).toBe("Copied");
    expect(wrapper.get("#pac-copy-btn").text()).toBe("Copied");
    expect(ariaOf(wrapper, "#socks-copy-btn")).toBe(HOST_COPY_ARIA);
    expect(wrapper.get("#socks-copy-btn").text()).toBe("Copy address");
  });

  it("after a click on the proxy address its label is \"Copied\", the PAC button's is unchanged", async () => {
    trapClipboard();
    const { wrapper } = await mountExpanded(MAC_CHROME);
    await wrapper.get("#socks-copy-btn").trigger("click");
    await flushPromises();
    expect(ariaOf(wrapper, "#socks-copy-btn")).toBe("Copied");
    expect(ariaOf(wrapper, "#pac-copy-btn")).toBe(PAC_COPY_ARIA);
  });

  it("after two seconds \"Copied\" fades and the label returns", async () => {
    trapClipboard();
    const { wrapper } = await mountExpanded(MAC_CHROME);
    await wrapper.get("#pac-copy-btn").trigger("click");
    await flushPromises();
    await vi.advanceTimersByTimeAsync(2000);
    expect(ariaOf(wrapper, "#pac-copy-btn")).toBe(PAC_COPY_ARIA);
  });

  it("no clipboard: \"Copied\" is not announced, the label is unchanged", async () => {
    Object.defineProperty(navigator, "clipboard", { value: undefined, configurable: true });
    const { wrapper } = await mountExpanded(MAC_CHROME);
    await wrapper.get("#pac-copy-btn").trigger("click");
    await flushPromises();
    expect(ariaOf(wrapper, "#pac-copy-btn")).toBe(PAC_COPY_ARIA);
  });

  it("\"Copy port\" has no aria-label attribute before or after copying — the name comes from the text", async () => {
    trapClipboard();
    const { wrapper } = await mountExpanded(MAC_CHROME);
    expect(wrapper.get("#socks-copy-port-btn").attributes("aria-label")).toBeUndefined();
    await wrapper.get("#socks-copy-port-btn").trigger("click");
    await flushPromises();
    expect(wrapper.get("#socks-copy-port-btn").text()).toBe("Copied");
    expect(wrapper.get("#socks-copy-port-btn").attributes("aria-label")).toBeUndefined();
  });

  it("\"Copy\" for other devices on the network has no aria-label attribute", () => {
    const lan = [{ addr: "192.168.1.7:1080", iface: "br-lan" }];
    const { wrapper } = mountWith(MAC_CHROME, baseState({ socks: { enabled: true, listen: LISTEN, pac_url: PAC_URL, access: "localnet", lan } }));
    expect(wrapper.get("#socks-copy-lan-btn").text()).toBe("Copy");
    expect(wrapper.get("#socks-copy-lan-btn").attributes("aria-label")).toBeUndefined();
  });
});

// aria-label of the button; a missing attribute is an error (otherwise comparing "undefined with undefined" passes vacuously)
function ariaOf(wrapper, selector) {
  const label = wrapper.get(selector).attributes("aria-label");
  expect(label, "no aria-label on " + selector).toBeTypeOf("string");
  return label;
}

function trapClipboard() {
  Object.defineProperty(navigator, "clipboard", { value: { writeText: async () => {} }, configurable: true });
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
